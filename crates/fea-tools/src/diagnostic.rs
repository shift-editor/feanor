//! Diagnostics, rules, fixes, and positions.
//!
//! Every range in this module is a byte range into the linted text. Editors and
//! the Language Server Protocol count lines and UTF-16 code units instead; use
//! [`LineIndex`] to convert.

use std::fmt;
use std::ops::Range;

/// How serious a [`Diagnostic`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Severity {
    /// The feature file won't compile, or compiles to something the author
    /// can't have meant.
    Error,
    /// Probably a mistake, but valid feature syntax.
    Warning,
    /// Worth knowing; often harmless (an unused class).
    Hint,
}

/// A lint rule. Each has a stable short code (`G001`) and a kebab-case name
/// (`glyph-not-in-font`); either can be used to select or suppress it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Rule {
    /// A parse error or warning reported by fea-rs.
    Syntax,
    /// A glyph name that isn't in the font's glyph list.
    GlyphNotInFont,
    /// `sub @A by @B` where the classes have different lengths.
    ClassLengthMismatch,
    /// The same glyph twice in one class.
    DuplicateGlyphInClass,
    /// A class that's used but never defined (project linting only).
    UndefinedClass,
    /// A class that's defined but never used.
    UnusedClass,
    /// A lookup that's referenced but never defined (project linting only).
    UndefinedLookup,
    /// A standalone lookup that's never referenced, so it never runs.
    UnusedLookup,
    /// A feature tag that isn't in the OpenType registry.
    UnknownFeatureTag,
    /// An invisible or non-ASCII character outside comments and strings.
    InvisibleCharacter,
    /// A problem assembling a project: a missing include, an include cycle,
    /// an unbalanced `#ifdef`.
    Project,
}

impl Rule {
    /// Every rule, in code order.
    pub const ALL: [Rule; 11] = [
        Rule::Syntax,
        Rule::GlyphNotInFont,
        Rule::ClassLengthMismatch,
        Rule::DuplicateGlyphInClass,
        Rule::UndefinedClass,
        Rule::UnusedClass,
        Rule::UndefinedLookup,
        Rule::UnusedLookup,
        Rule::UnknownFeatureTag,
        Rule::InvisibleCharacter,
        Rule::Project,
    ];

    /// The stable short code. The letter is the rule's family: `E` syntax,
    /// `G` glyphs, `C` classes, `L` lookups, `T` tags, `X` characters, `P` projects.
    pub const fn code(self) -> &'static str {
        match self {
            Rule::Syntax => "E001",
            Rule::GlyphNotInFont => "G001",
            Rule::ClassLengthMismatch => "C001",
            Rule::DuplicateGlyphInClass => "C002",
            Rule::UndefinedClass => "C003",
            Rule::UnusedClass => "C004",
            Rule::UndefinedLookup => "L001",
            Rule::UnusedLookup => "L002",
            Rule::UnknownFeatureTag => "T001",
            Rule::InvisibleCharacter => "X001",
            Rule::Project => "P001",
        }
    }

    /// The stable kebab-case name.
    pub const fn name(self) -> &'static str {
        match self {
            Rule::Syntax => "syntax",
            Rule::GlyphNotInFont => "glyph-not-in-font",
            Rule::ClassLengthMismatch => "class-length-mismatch",
            Rule::DuplicateGlyphInClass => "duplicate-glyph-in-class",
            Rule::UndefinedClass => "undefined-class",
            Rule::UnusedClass => "unused-class",
            Rule::UndefinedLookup => "undefined-lookup",
            Rule::UnusedLookup => "unused-lookup",
            Rule::UnknownFeatureTag => "unknown-feature-tag",
            Rule::InvisibleCharacter => "invisible-character",
            Rule::Project => "project",
        }
    }

    /// One line describing what the rule reports.
    pub const fn summary(self) -> &'static str {
        match self {
            Rule::Syntax => "fea-rs parse error or warning",
            Rule::GlyphNotInFont => "glyph name not in the font's glyph list",
            Rule::ClassLengthMismatch => "`sub @A by @B` with classes of different lengths",
            Rule::DuplicateGlyphInClass => "the same glyph twice in one class",
            Rule::UndefinedClass => "class used but never defined (project linting)",
            Rule::UnusedClass => "class defined but never used",
            Rule::UndefinedLookup => "lookup referenced but never defined (project linting)",
            Rule::UnusedLookup => "standalone lookup never referenced",
            Rule::UnknownFeatureTag => "feature tag not in the OpenType registry",
            Rule::InvisibleCharacter => {
                "invisible or non-ASCII character outside comments and strings"
            }
            Rule::Project => "missing include, include cycle, or unbalanced #ifdef",
        }
    }

    /// The rule's documentation in Markdown: what it reports, why it matters,
    /// an example, and what its fix does. `fea rule CODE` prints this.
    pub const fn explanation(self) -> &'static str {
        match self {
            Rule::Syntax => include_str!("../rules/E001.md"),
            Rule::GlyphNotInFont => include_str!("../rules/G001.md"),
            Rule::ClassLengthMismatch => include_str!("../rules/C001.md"),
            Rule::DuplicateGlyphInClass => include_str!("../rules/C002.md"),
            Rule::UndefinedClass => include_str!("../rules/C003.md"),
            Rule::UnusedClass => include_str!("../rules/C004.md"),
            Rule::UndefinedLookup => include_str!("../rules/L001.md"),
            Rule::UnusedLookup => include_str!("../rules/L002.md"),
            Rule::UnknownFeatureTag => include_str!("../rules/T001.md"),
            Rule::InvisibleCharacter => include_str!("../rules/X001.md"),
            Rule::Project => include_str!("../rules/P001.md"),
        }
    }

    /// The rule with this code or name (`"G001"` or `"glyph-not-in-font"`).
    pub fn from_code_or_name(s: &str) -> Option<Rule> {
        Rule::ALL
            .into_iter()
            .find(|r| r.code().eq_ignore_ascii_case(s) || r.name() == s)
    }

    /// Whether `selector` picks this rule: its code, its family letter or a
    /// prefix of its code (`C`, `C00`), its name, or `ALL`.
    pub fn matches(self, selector: &str) -> bool {
        let selector = selector.trim();
        !selector.is_empty()
            && (selector.eq_ignore_ascii_case("ALL")
                || selector == self.name()
                || self.code().starts_with(&selector.to_ascii_uppercase()))
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl serde::Serialize for Rule {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

/// Whether a [`Fix`] keeps the compiled font's behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Applicability {
    /// Changes nothing the shaper sees: removing a repeated glyph from a class,
    /// deleting a stray invisible character. Safe to apply without review.
    Safe,
    /// Changes what the font does, such as renaming a glyph or a feature to a
    /// suggestion. Apply only after checking that the suggestion is right.
    Unsafe,
}

/// Replace `range` (bytes) of the linted text with `new_text`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TextEdit {
    /// Bytes of the linted text to replace.
    pub range: Range<usize>,
    /// The replacement; empty to delete.
    pub new_text: String,
}

/// A set of edits that resolves a [`Diagnostic`]. The edits never overlap.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct Fix {
    /// What the fix does, for a quick-fix menu ("Replace with 'agrave'").
    pub title: String,
    /// The edits, in no particular order.
    pub edits: Vec<TextEdit>,
    /// Whether the fix can be applied without review.
    pub applicability: Applicability,
}

impl Fix {
    pub(crate) fn new(
        title: impl Into<String>,
        edits: Vec<TextEdit>,
        applicability: Applicability,
    ) -> Fix {
        Fix {
            title: title.into(),
            edits,
            applicability,
        }
    }
}

/// Another location that explains a diagnostic (the first occurrence of a duplicate).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Related {
    /// Byte range in the same text as the diagnostic.
    pub range: Range<usize>,
    /// What the location is, for display ("first occurrence").
    pub label: String,
}

/// One problem found by [`lint`](crate::lint()).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct Diagnostic {
    /// The rule that reported it.
    pub rule: Rule,
    /// Usually the rule's severity; [`Rule::Syntax`] carries fea-rs's own.
    pub severity: Severity,
    /// Byte range of the offending text.
    pub range: Range<usize>,
    /// One sentence for a person, naming the offending text.
    pub message: String,
    /// Ways to resolve it, best first. Empty when there's no mechanical fix.
    pub fixes: Vec<Fix>,
    /// Another location that explains it, if any.
    pub related: Option<Related>,
}

impl Diagnostic {
    pub(crate) fn new(
        rule: Rule,
        severity: Severity,
        range: Range<usize>,
        message: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic {
            rule,
            severity,
            range,
            message: message.into(),
            fixes: Vec::new(),
            related: None,
        }
    }

    /// The same diagnostic with every range moved `by` bytes later.
    pub(crate) fn shifted(&self, by: usize) -> Diagnostic {
        let mut d = self.clone();
        let shift = |r: &mut Range<usize>| *r = r.start + by..r.end + by;
        shift(&mut d.range);
        for fix in &mut d.fixes {
            for e in &mut fix.edits {
                shift(&mut e.range);
            }
        }
        if let Some(related) = &mut d.related {
            shift(&mut related.range);
        }
        d
    }
}

/// A zero-based line and column, with the column counted in UTF-16 code
/// units, as the Language Server Protocol and CodeMirror count them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct Position {
    /// Zero-based line.
    pub line: u32,
    /// Zero-based column in UTF-16 code units.
    pub character: u32,
}

/// Converts byte offsets in one text to [`Position`]s.
///
/// Building the index is linear in the text; each lookup is a binary search
/// plus a scan of one line.
#[derive(Clone, Debug)]
pub struct LineIndex<'a> {
    text: &'a str,
    /// Byte offset where each line starts.
    starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    /// Index the line starts of `text`.
    pub fn new(text: &'a str) -> LineIndex<'a> {
        let starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        LineIndex { text, starts }
    }

    /// The position of byte `offset`. Offsets past the end, or inside a
    /// multi-byte character, are moved back to the nearest character boundary.
    pub fn position(&self, offset: usize) -> Position {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.starts.partition_point(|&s| s <= offset) - 1;
        let character = self.text[self.starts[line]..offset].encode_utf16().count();
        // a feature file with four billion lines or columns is not a feature file
        let to_u32 = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Position {
            line: to_u32(line),
            character: to_u32(character),
        }
    }
}

/// Apply the first fix of each diagnostic that `allowed` admits, skipping any
/// fix whose edits overlap one already applied. Returns the new text.
pub fn apply_fixes(source: &str, diagnostics: &[Diagnostic], allowed: Applicability) -> String {
    let mut edits: Vec<&TextEdit> = Vec::new();
    for d in diagnostics {
        let Some(fix) = d.fixes.iter().find(|f| f.applicability <= allowed) else {
            continue;
        };
        let overlaps = fix.edits.iter().any(|e| {
            edits
                .iter()
                .any(|x| e.range.start < x.range.end && x.range.start < e.range.end)
        });
        if !overlaps {
            edits.extend(&fix.edits);
        }
    }
    edits.sort_by_key(|e| e.range.start);
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for e in edits {
        out.push_str(&source[at..e.range.start]);
        out.push_str(&e.new_text);
        at = e.range.end;
    }
    out.push_str(&source[at..]);
    out
}

/// Drop diagnostics suppressed by a `# noqa` comment on their line. A bare
/// `# noqa` suppresses everything on the line; `# noqa: G001, unused-class`
/// only those rules. Returns how many were dropped.
pub fn apply_noqa(source: &str, diagnostics: &mut Vec<Diagnostic>) -> usize {
    let before = diagnostics.len();
    diagnostics.retain(|d| {
        let start = d.range.start.min(source.len());
        let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
        let line = source[line_start..].lines().next().unwrap_or("");
        let Some(at) = line.find("# noqa") else {
            return true;
        };
        let rest = line[at + "# noqa".len()..].trim_start();
        let Some(list) = rest.strip_prefix(':') else {
            return false;
        };
        !list
            .split(',')
            .filter_map(|s| s.split_whitespace().next())
            .any(|sel| d.rule.matches(sel))
    });
    before - diagnostics.len()
}

/// Which rules to report, Ruff-style: everything `select`ed, minus everything `ignore`d.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// Selectors to report. Empty means every rule.
    pub select: Vec<String>,
    /// Selectors to skip, applied after `select`.
    pub ignore: Vec<String>,
}

impl Selection {
    /// Whether `rule` is reported: selected (or nothing selected) and not ignored.
    /// See [`Rule::matches`] for selector syntax.
    pub fn allows(&self, rule: Rule) -> bool {
        let selected = self.select.is_empty() || self.select.iter().any(|s| rule.matches(s));
        selected && !self.ignore.iter().any(|s| rule.matches(s))
    }

    /// Keep only the diagnostics whose rule this selection allows.
    pub fn retain(&self, diagnostics: &mut Vec<Diagnostic>) {
        diagnostics.retain(|d| self.allows(d.rule));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_names_are_unique_and_round_trip() {
        for rule in Rule::ALL {
            assert_eq!(Rule::from_code_or_name(rule.code()), Some(rule));
            assert_eq!(Rule::from_code_or_name(rule.name()), Some(rule));
            assert!(
                rule.explanation()
                    .starts_with(&format!("# {}", rule.code())),
                "{} docs heading",
                rule.code()
            );
        }
        let codes: std::collections::HashSet<_> = Rule::ALL.iter().map(|r| r.code()).collect();
        assert_eq!(codes.len(), Rule::ALL.len());
    }

    #[test]
    fn selectors_match_codes_families_and_names() {
        assert!(Rule::UnusedClass.matches("C004"));
        assert!(Rule::UnusedClass.matches("C"));
        assert!(Rule::UnusedClass.matches("c00"));
        assert!(Rule::UnusedClass.matches("unused-class"));
        assert!(Rule::UnusedClass.matches("ALL"));
        assert!(!Rule::UnusedClass.matches("G"));
        assert!(!Rule::UnusedClass.matches(""));
    }

    #[test]
    fn positions_count_utf16_units() {
        let text = "a\n\u{1F600}b\nc";
        let index = LineIndex::new(text);
        assert_eq!(
            index.position(0),
            Position {
                line: 0,
                character: 0
            }
        );
        assert_eq!(
            index.position(2),
            Position {
                line: 1,
                character: 0
            }
        );
        // the emoji is four bytes and two UTF-16 units
        assert_eq!(
            index.position(6),
            Position {
                line: 1,
                character: 2
            }
        );
        assert_eq!(
            index.position(text.len()),
            Position {
                line: 2,
                character: 1
            }
        );
    }

    #[test]
    fn overlapping_fixes_are_applied_once() {
        let mut a = Diagnostic::new(Rule::Syntax, Severity::Error, 0..1, "a");
        a.fixes.push(Fix::new(
            "x",
            vec![TextEdit {
                range: 0..2,
                new_text: "X".into(),
            }],
            Applicability::Safe,
        ));
        let mut b = a.clone();
        b.fixes[0].edits[0] = TextEdit {
            range: 1..3,
            new_text: "Y".into(),
        };
        assert_eq!(apply_fixes("abcd", &[a, b], Applicability::Safe), "Xcd");
    }

    #[test]
    fn unsafe_fixes_need_permission() {
        let mut d = Diagnostic::new(Rule::GlyphNotInFont, Severity::Error, 0..1, "a");
        d.fixes.push(Fix::new(
            "x",
            vec![TextEdit {
                range: 0..1,
                new_text: "b".into(),
            }],
            Applicability::Unsafe,
        ));
        assert_eq!(
            apply_fixes("a", std::slice::from_ref(&d), Applicability::Safe),
            "a"
        );
        assert_eq!(apply_fixes("a", &[d], Applicability::Unsafe), "b");
    }
}
