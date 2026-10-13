//! JavaScript bindings for [feanor](https://docs.rs/feanor), built with
//! wasm-bindgen.
//!
//! Every position crossing the boundary is a UTF-16 offset into the string
//! JavaScript passed in (what `String.prototype.slice` and CodeMirror use).
//! Diagnostics also carry zero-based line/character positions for the
//! Language Server Protocol. Options and results are plain JavaScript objects;
//! their TypeScript types are declared below.

use std::ops::Range;

use feanor::{
    Applicability, Diagnostic, FormatOptions, GlyphNames, Indent, LineIndex, LintOptions, Rule,
    Selection,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r#"
export interface FormatOptions {
  /** Width to pack glyph classes to and wrap long rules at. Default 100. */
  lineWidth?: number;
  /** Spaces per level, "tab", or "auto" (the text's own indent). Default "auto". */
  indent?: number | "tab" | "auto";
  /** Re-align runs of statements the author aligned. Default true. */
  keepAlignment?: boolean;
}

export interface Formatted {
  text: string;
  /** Ranges of the input left exactly as written (they don't parse, or `# fmt: off`). */
  untouched: Span[];
  /** The input was formatted as the inside of a `feature` block. */
  featureBody: boolean;
}

export interface LintOptions {
  /** The font's glyph names; enables G001 glyph-not-in-font. */
  glyphs?: string[];
  /** The text is a whole project, so undefined classes and lookups are errors. */
  project?: boolean;
  /** Rules to report: codes ("G001"), families ("C") or names. Empty: all. */
  select?: string[];
  /** Rules to skip. */
  ignore?: string[];
  /** Honour `# noqa` comments. Default true. */
  noqa?: boolean;
}

export interface FixOptions extends LintOptions {
  /** Also apply fixes that change the font (renames). Default false. */
  unsafe?: boolean;
}

/** A UTF-16 range: `text.slice(from, to)`. */
export interface Span { from: number; to: number; }

/** Zero-based line, and column in UTF-16 code units (Language Server Protocol). */
export interface Position { line: number; character: number; }

export interface Edit { from: number; to: number; insert: string; }

export interface Fix {
  title: string;
  edits: Edit[];
  applicability: "safe" | "unsafe";
}

export interface Diagnostic {
  /** Stable short code, e.g. "G001". */
  code: string;
  /** Stable kebab-case name, e.g. "glyph-not-in-font". */
  rule: string;
  severity: "error" | "warning" | "hint";
  message: string;
  from: number;
  to: number;
  start: Position;
  end: Position;
  fixes: Fix[];
  related?: Span & { label: string };
}

export interface RuleInfo { code: string; name: string; summary: string; }
"#;

/// Maps between UTF-8 byte offsets and UTF-16 offsets in one text.
///
/// Records every non-ASCII character; between them, one byte is one UTF-16 unit.
struct Offsets {
    /// `(start byte, end byte, UTF-16 offset at the end)` of each non-ASCII character, in order.
    wide: Vec<(usize, usize, usize)>,
    len: usize,
}

impl Offsets {
    fn new(text: &str) -> Offsets {
        let mut wide = Vec::new();
        let mut utf16 = 0;
        for (byte, c) in text.char_indices() {
            utf16 += c.len_utf16();
            if !c.is_ascii() {
                wide.push((byte, byte + c.len_utf8(), utf16));
            }
        }
        Offsets {
            wide,
            len: text.len(),
        }
    }

    fn to_utf16(&self, byte: usize) -> usize {
        let i = self.wide.partition_point(|&(_, end, _)| end <= byte);
        match i.checked_sub(1).map(|i| self.wide[i]) {
            Some((_, end, u)) => u + (byte - end),
            None => byte,
        }
    }

    /// The byte offset of a UTF-16 offset, clamped to the text. An offset in
    /// the middle of a surrogate pair moves to the end of that character.
    fn to_byte(&self, utf16: usize) -> usize {
        let i = self.wide.partition_point(|&(_, _, u)| u <= utf16);
        let byte = match i.checked_sub(1).map(|i| self.wide[i]) {
            Some((_, end, u)) => end + (utf16 - u),
            None => utf16,
        };
        match self.wide.get(i) {
            // past the start of the next wide character: inside it
            Some(&(start, end, _)) if byte > start => end,
            _ => byte.min(self.len),
        }
    }

    fn span(&self, r: &Range<usize>) -> Span {
        Span {
            from: self.to_utf16(r.start),
            to: self.to_utf16(r.end),
        }
    }
}

#[derive(Serialize)]
struct Span {
    from: usize,
    to: usize,
}

#[derive(Serialize)]
struct JsEdit {
    from: usize,
    to: usize,
    insert: String,
}

#[derive(Serialize)]
struct JsFix {
    title: String,
    edits: Vec<JsEdit>,
    applicability: Applicability,
}

#[derive(Serialize)]
struct JsRelated {
    from: usize,
    to: usize,
    label: String,
}

#[derive(Serialize)]
struct JsDiagnostic {
    code: &'static str,
    rule: &'static str,
    severity: feanor::Severity,
    message: String,
    from: usize,
    to: usize,
    start: feanor::Position,
    end: feanor::Position,
    fixes: Vec<JsFix>,
    #[serde(skip_serializing_if = "Option::is_none")]
    related: Option<JsRelated>,
}

fn to_js_diagnostics(text: &str, diagnostics: &[Diagnostic]) -> Vec<JsDiagnostic> {
    let offsets = Offsets::new(text);
    let lines = LineIndex::new(text);
    diagnostics
        .iter()
        .map(|d| {
            let span = offsets.span(&d.range);
            JsDiagnostic {
                code: d.rule.code(),
                rule: d.rule.name(),
                severity: d.severity,
                message: d.message.clone(),
                from: span.from,
                to: span.to,
                start: lines.position(d.range.start),
                end: lines.position(d.range.end),
                fixes: d
                    .fixes
                    .iter()
                    .map(|f| JsFix {
                        title: f.title.clone(),
                        edits: f
                            .edits
                            .iter()
                            .map(|e| {
                                let s = offsets.span(&e.range);
                                JsEdit {
                                    from: s.from,
                                    to: s.to,
                                    insert: e.new_text.clone(),
                                }
                            })
                            .collect(),
                        applicability: f.applicability,
                    })
                    .collect(),
                related: d.related.as_ref().map(|r| {
                    let s = offsets.span(&r.range);
                    JsRelated {
                        from: s.from,
                        to: s.to,
                        label: r.label.clone(),
                    }
                }),
            }
        })
        .collect()
}

/// Serialize to a plain JavaScript object.
fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
    let json = serde_json::to_string(value).map_err(|e| JsError::new(&e.to_string()))?;
    js_sys::JSON::parse(&json).map_err(|_| JsError::new("couldn't build the result"))
}

/// Read options from a JavaScript object; `undefined` and `null` give the defaults.
fn from_js<T: for<'de> Deserialize<'de> + Default>(
    value: &JsValue,
    what: &str,
) -> Result<T, JsError> {
    if value.is_undefined() || value.is_null() {
        return Ok(T::default());
    }
    let json = js_sys::JSON::stringify(value)
        .ok()
        .and_then(|s| s.as_string())
        .ok_or_else(|| JsError::new(&format!("{what} must be a plain object")))?;
    serde_json::from_str(&json).map_err(|e| JsError::new(&format!("invalid {what}: {e}")))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
struct JsFormatOptions {
    line_width: Option<usize>,
    indent: Option<serde_json::Value>,
    keep_alignment: Option<bool>,
}

impl JsFormatOptions {
    fn resolve(&self) -> Result<FormatOptions, JsError> {
        let mut options = FormatOptions::default();
        if let Some(w) = self.line_width {
            options.line_width = w;
        }
        if let Some(i) = &self.indent {
            options.indent = match (i.as_u64(), i.as_str()) {
                (Some(n), _) if n > 0 => Indent::Spaces(usize::try_from(n).unwrap_or(4)),
                (_, Some("tab")) => Indent::Tab,
                (_, Some("auto")) => Indent::Auto,
                _ => {
                    return Err(JsError::new(
                        "indent must be a number of spaces, \"tab\" or \"auto\"",
                    ));
                }
            };
        }
        if let Some(k) = self.keep_alignment {
            options.keep_alignment = k;
        }
        Ok(options)
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
struct JsLintOptions {
    glyphs: Option<Vec<String>>,
    project: bool,
    select: Vec<String>,
    ignore: Vec<String>,
    noqa: Option<bool>,
    #[serde(rename = "unsafe")]
    unsafe_fixes: bool,
}

impl JsLintOptions {
    fn glyphs(&self) -> Option<GlyphNames> {
        self.glyphs
            .as_ref()
            .map(|g| g.iter().map(String::as_str).collect())
    }

    fn selection(&self) -> Selection {
        Selection {
            select: self.select.clone(),
            ignore: self.ignore.clone(),
        }
    }

    /// Selection and `# noqa`, applied after linting.
    fn filter(&self, text: &str, diagnostics: &mut Vec<Diagnostic>) {
        self.selection().retain(diagnostics);
        if self.noqa.unwrap_or(true) {
            feanor::apply_noqa(text, diagnostics);
        }
    }
}

fn lint_options<'a>(glyphs: Option<&'a GlyphNames>, project: bool) -> LintOptions<'a> {
    let options = LintOptions::default().whole_project(project);
    match glyphs {
        Some(g) => options.with_glyphs(g),
        None => options,
    }
}

fn run_lint(source: &str, options: &JsLintOptions) -> Vec<Diagnostic> {
    let glyphs = options.glyphs();
    let mut diagnostics = feanor::lint(source, &lint_options(glyphs.as_ref(), options.project));
    options.filter(source, &mut diagnostics);
    diagnostics
}

/// Format feature code. Accepts a whole file or a feature body.
#[wasm_bindgen(unchecked_return_type = "Formatted")]
pub fn format(
    source: &str,
    #[wasm_bindgen(unchecked_param_type = "FormatOptions")] options: JsValue,
) -> Result<JsValue, JsError> {
    let options: JsFormatOptions = from_js(&options, "format options")?;
    let out = feanor::format(source, &options.resolve()?);
    let offsets = Offsets::new(source);
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Out {
        text: String,
        untouched: Vec<Span>,
        feature_body: bool,
    }
    to_js(&Out {
        untouched: out.untouched.iter().map(|r| offsets.span(r)).collect(),
        text: out.text,
        feature_body: out.feature_body,
    })
}

/// Lint feature code. Accepts a whole file or a feature body.
#[wasm_bindgen(unchecked_return_type = "Diagnostic[]")]
pub fn lint(
    source: &str,
    #[wasm_bindgen(unchecked_param_type = "LintOptions")] options: JsValue,
) -> Result<JsValue, JsError> {
    let options: JsLintOptions = from_js(&options, "lint options")?;
    to_js(&to_js_diagnostics(source, &run_lint(source, &options)))
}

/// Apply the first fix of each diagnostic: safe fixes only, or all with `unsafe: true`.
#[wasm_bindgen]
pub fn fix(
    source: &str,
    #[wasm_bindgen(unchecked_param_type = "FixOptions")] options: JsValue,
) -> Result<String, JsError> {
    let options: JsLintOptions = from_js(&options, "fix options")?;
    let allowed = if options.unsafe_fixes {
        Applicability::Unsafe
    } else {
        Applicability::Safe
    };
    Ok(feanor::apply_fixes(
        source,
        &run_lint(source, &options),
        allowed,
    ))
}

/// Every rule's code, name and one-line summary.
#[wasm_bindgen(unchecked_return_type = "RuleInfo[]")]
pub fn rules() -> Result<JsValue, JsError> {
    #[derive(Serialize)]
    struct Info {
        code: &'static str,
        name: &'static str,
        summary: &'static str,
    }
    let all: Vec<Info> = Rule::ALL
        .iter()
        .map(|r| Info {
            code: r.code(),
            name: r.name(),
            summary: r.summary(),
        })
        .collect();
    to_js(&all)
}

/// A rule's documentation in Markdown, by code or name; `undefined` if there's no such rule.
#[wasm_bindgen]
pub fn explain(code_or_name: &str) -> Option<String> {
    Rule::from_code_or_name(code_or_name).map(|r| r.explanation().to_string())
}

/// The feanor version these bindings were built from.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// A document that stays linted as it's edited: each edit re-parses only the
/// top-level items it touches. Feature bodies aren't detected here; wrap a
/// body in a `feature` block, or use [`lint`] for one-off checks.
#[wasm_bindgen]
pub struct Document {
    inner: feanor::Incremental,
    options: JsLintOptions,
}

#[wasm_bindgen]
impl Document {
    /// Start linting `text`. The options last as long as the document.
    #[wasm_bindgen(constructor)]
    pub fn new(
        text: String,
        #[wasm_bindgen(unchecked_param_type = "LintOptions")] options: JsValue,
    ) -> Result<Document, JsError> {
        let options: JsLintOptions = from_js(&options, "lint options")?;
        let glyphs = options.glyphs();
        let inner = feanor::Incremental::new(text, lint_options(glyphs.as_ref(), options.project));
        Ok(Document { inner, options })
    }

    /// Replace UTF-16 range `from..to` with `insert`, as a CodeMirror change does.
    pub fn edit(&mut self, from: usize, to: usize, insert: &str) -> Result<(), JsError> {
        if from > to {
            return Err(JsError::new("edit: `from` is after `to`"));
        }
        let offsets = Offsets::new(self.inner.text());
        let (start, end) = (offsets.to_byte(from), offsets.to_byte(to));
        self.inner.edit(start..end, insert);
        Ok(())
    }

    /// The current text.
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        self.inner.text().to_string()
    }

    /// Diagnostics for the current text.
    #[wasm_bindgen(unchecked_return_type = "Diagnostic[]")]
    pub fn diagnostics(&mut self) -> Result<JsValue, JsError> {
        let mut diagnostics = self.inner.diagnostics();
        let text = self.inner.text().to_string();
        self.options.filter(&text, &mut diagnostics);
        to_js(&to_js_diagnostics(&text, &diagnostics))
    }
}

#[cfg(test)]
mod tests {
    use super::Offsets;

    #[test]
    fn offsets_round_trip_through_non_ascii_text() {
        // "a" (1 byte, 1 unit), "é" (2, 1), "😀" (4, 2), "b" (1, 1)
        let text = "aé😀b";
        let o = Offsets::new(text);
        assert_eq!(o.to_utf16(0), 0);
        assert_eq!(o.to_utf16(1), 1);
        assert_eq!(o.to_utf16(3), 2);
        assert_eq!(o.to_utf16(7), 4);
        assert_eq!(o.to_utf16(8), 5);
        for (byte, utf16) in [(0, 0), (1, 1), (3, 2), (7, 4), (8, 5)] {
            assert_eq!(o.to_byte(utf16), byte);
        }
        // the middle of the emoji's surrogate pair moves to the end of the emoji
        assert_eq!(o.to_byte(3), 7);
        assert_eq!(o.to_byte(99), text.len());
    }
}
