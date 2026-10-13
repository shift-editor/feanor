//! Lint spike: rules over fea-rs's syntax tree, plus the glyph list when known.
//!
//! Output is LSP-shaped: each diagnostic has a rule id, severity, a byte range
//! (converted to line / UTF-16 column at the edge), a message, and optional
//! fixes made of text edits.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;

use fea_rs::{Kind, Node, NodeOrToken};

use crate::glyphs::GlyphNames;

use crate::diagnostic::{Applicability, Diagnostic, Fix, Related, Rule, Severity, TextEdit};
use crate::fmt::{parse_glyph_map, parse_with_diagnostics};

/// What the linter knows besides the text.
///
/// ```
/// use feanor::{GlyphNames, LintOptions};
///
/// let glyphs: GlyphNames = ["a", "b"].into_iter().collect();
/// let options = LintOptions::default().with_glyphs(&glyphs).whole_project(true);
/// assert!(options.project);
/// ```
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct LintOptions<'a> {
    /// The font's glyph names. [`Rule::GlyphNotInFont`] runs only when they're known.
    pub glyphs: Option<&'a GlyphNames>,
    /// The text is a whole project with every include resolved, so a name
    /// that's never defined is an error rather than a definition elsewhere.
    /// Enables [`Rule::UndefinedClass`] and [`Rule::UndefinedLookup`].
    pub project: bool,
}

impl<'a> LintOptions<'a> {
    /// Check glyph names against `glyphs`.
    pub fn with_glyphs(mut self, glyphs: &'a GlyphNames) -> Self {
        self.glyphs = Some(glyphs);
        self
    }

    /// Treat the text as a whole project; see [`LintOptions::project`].
    pub fn whole_project(mut self, project: bool) -> Self {
        self.project = project;
        self
    }
}

/// "Did you mean" suggestions already computed, by misspelled glyph name.
pub(crate) type Suggestions = HashMap<String, Option<String>>;

/// A token or node with its absolute byte range (fea-rs only fills positions in for multi-file parses).
struct Item<'a> {
    kind: Kind,
    text: Option<&'a str>,
    range: Range<usize>,
    /// Nesting depth (root children are 0).
    depth: usize,
    /// Kind of the enclosing node (`Kind::Eof` at the root).
    parent_kind: Kind,
    /// Index of the enclosing node in the item list (`usize::MAX` at the root).
    parent: usize,
}

fn index<'a>(root: &'a Node) -> Vec<Item<'a>> {
    fn walk<'a>(
        node: &'a Node,
        pos: &mut usize,
        depth: usize,
        parent: usize,
        out: &mut Vec<Item<'a>>,
    ) {
        let parent_kind = if parent == usize::MAX {
            Kind::Eof
        } else {
            node.kind()
        };
        for child in node.iter_children() {
            match child {
                NodeOrToken::Node(n) => {
                    let start = *pos;
                    let at = out.len();
                    out.push(Item {
                        kind: n.kind(),
                        text: None,
                        range: start..start,
                        depth,
                        parent_kind,
                        parent,
                    });
                    walk(n, pos, depth + 1, at, out);
                    out[at].range = start..*pos;
                }
                NodeOrToken::Token(t) => {
                    let start = *pos;
                    *pos += t.text.len();
                    out.push(Item {
                        kind: t.kind,
                        text: Some(t.text.as_str()),
                        range: start..*pos,
                        depth,
                        parent_kind,
                        parent,
                    });
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut 0, 0, usize::MAX, &mut out);
    out
}

/// Lint feature code.
///
/// `source` can be a whole feature file or a feature body (the inside of a
/// `feature` block, as Glyphs and many build systems store it): when the text
/// only parses wrapped in a `feature` block, it's linted that way and every
/// range still refers to `source`.
///
/// Diagnostics are sorted by position. Select or suppress them afterwards
/// with [`Selection`](crate::Selection) and [`apply_noqa`](crate::apply_noqa).
///
/// ```
/// use feanor::{lint, LintOptions, Rule};
///
/// let source = "@lc = [a b a];\nfeature liga { sub @lc by f_i; } liga;\n";
/// let diagnostics = lint(source, &LintOptions::default());
/// assert_eq!(diagnostics[0].rule, Rule::DuplicateGlyphInClass);
/// assert_eq!(&source[diagnostics[0].range.clone()], "a");
/// ```
pub fn lint(source: &str, options: &LintOptions) -> Vec<Diagnostic> {
    const OPEN: &str = "feature zzzz {\n";
    let mut cache = Suggestions::new();
    let as_file = lint_document(source, options, &mut cache);
    let syntax_errors = |ds: &[Diagnostic]| {
        ds.iter()
            .filter(|d| d.rule == Rule::Syntax && d.severity == Severity::Error)
            .count()
    };
    if syntax_errors(&as_file) == 0 {
        return as_file;
    }
    let wrapped = format!("{OPEN}{source}\n}} zzzz;\n");
    let as_body = lint_document(&wrapped, options, &mut cache);
    if syntax_errors(&as_body) >= syntax_errors(&as_file) {
        return as_file;
    }
    let inside = OPEN.len()..OPEN.len() + source.len();
    let in_source = |r: &Range<usize>| inside.contains(&r.start) && r.end <= inside.end;
    let back = |r: &mut Range<usize>| *r = r.start - OPEN.len()..r.end - OPEN.len();
    as_body
        .into_iter()
        .filter(|d| in_source(&d.range))
        .map(|mut d| {
            back(&mut d.range);
            // a fix or related location that touches the wrapper can't be expressed in `source`
            d.fixes
                .retain(|f| f.edits.iter().all(|e| in_source(&e.range)));
            for fix in &mut d.fixes {
                for edit in &mut fix.edits {
                    back(&mut edit.range);
                }
            }
            d.related = d
                .related
                .take()
                .filter(|r| in_source(&r.range))
                .map(|mut r| {
                    back(&mut r.range);
                    r
                });
            d
        })
        .collect()
}

/// What one piece of a document (the whole file, or one top-level item while
/// editing) contributes. Ranges are relative to the start of the piece; the
/// rules that need the whole document run in [`combine`], over these facts only.
#[derive(Clone, Debug, Default)]
pub(crate) struct Facts {
    /// Diagnostics that depend only on this piece: syntax, unknown-feature-tag, invisible-character.
    local: Vec<Diagnostic>,
    missing_glyphs: Vec<(String, Range<usize>)>,
    /// Duplicate-glyph diagnostics, with the class name when the class is a definition
    /// (dropped later if that class is used on a replacement side anywhere).
    duplicates: Vec<(Option<String>, Diagnostic)>,
    replacement_classes: Vec<String>,
    class_defs: Vec<ClassDef>,
    mark_classes: Vec<(String, Range<usize>)>,
    /// (name, range, defined inside a feature block)
    lookup_defs: Vec<(String, Range<usize>, bool)>,
    class_uses: Vec<(String, Range<usize>)>,
    lookup_uses: Vec<(String, Range<usize>)>,
    sub_pairs: Vec<SubPair>,
}

/// A class definition: name, the name's range, and its members (None if not expandable).
type ClassDef = (String, Range<usize>, Option<Vec<Member>>);

#[derive(Clone, Debug)]
enum Member {
    Glyph(String),
    Class(String),
}

/// `sub <class> by <class>`, with both operands as unexpanded members
/// (`None` when an operand holds a range or a Glyphs predicate).
#[derive(Clone, Debug)]
struct SubPair {
    range: Range<usize>,
    target: Option<Vec<Member>>,
    replacement: Option<Vec<Member>>,
    left: String,
    right: String,
}

/// Lint `source` as one document, with no feature-body detection.
pub(crate) fn lint_document(
    source: &str,
    options: &LintOptions,
    cache: &mut Suggestions,
) -> Vec<Diagnostic> {
    let facts = analyze(source, options);
    combine(std::iter::once((0, &facts)), options, cache)
}

/// Parse one piece of text and collect its facts.
pub(crate) fn analyze(source: &str, ctx: &LintOptions) -> Facts {
    let map = parse_glyph_map(source);
    let (tree, parse_diagnostics) = parse_with_diagnostics(source, Some(&map));
    let items = index(tree.root());
    let mut facts = Facts::default();

    for d in parse_diagnostics.diagnostics() {
        let severity = if d.is_error() {
            Severity::Error
        } else {
            Severity::Warning
        };
        let span = d.span();
        let range = span.start.min(source.len())..span.end.min(source.len());
        facts.local.push(Diagnostic {
            rule: Rule::Syntax,
            severity,
            range,
            message: d.text().to_string(),
            fixes: Vec::new(),
            related: None,
        });
    }
    unknown_feature_tag(&items, &mut facts.local);
    invisible_characters(&items, source, &mut facts.local);
    if let Some(font) = ctx.glyphs {
        missing_glyphs(&items, font, &mut facts.missing_glyphs);
    }
    duplicate_candidates(&items, &mut facts);
    class_facts(&items, source, ctx.glyphs, &mut facts);
    definition_facts(&items, &mut facts);
    facts
}

/// Combine the facts of every piece (with its byte offset) into diagnostics.
pub(crate) fn combine<'f>(
    pieces: impl Iterator<Item = (usize, &'f Facts)> + Clone,
    ctx: &LintOptions,
    cache: &mut Suggestions,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (at, f) in pieces.clone() {
        out.extend(f.local.iter().map(|d| d.shifted(at)));
    }
    missing_glyph_diagnostics(pieces.clone(), ctx, cache, &mut out);

    let replacement_classes: HashSet<&str> = pieces
        .clone()
        .flat_map(|(_, f)| f.replacement_classes.iter().map(String::as_str))
        .collect();
    for (at, f) in pieces.clone() {
        for (defined, d) in &f.duplicates {
            if defined
                .as_deref()
                .is_some_and(|n| replacement_classes.contains(n))
            {
                continue;
            }
            out.push(d.shifted(at));
        }
    }

    class_length_mismatch(pieces.clone(), &mut out);
    definitions(pieces, ctx.project, &mut out);
    drop_covered_syntax_errors(&mut out);
    out.sort_by(|a, b| {
        (a.range.start, a.severity, a.rule, &a.message).cmp(&(
            b.range.start,
            b.severity,
            b.rule,
            &b.message,
        ))
    });
    out
}

// ---------------------------------------------------------------- glyph-not-in-font

fn missing_glyphs(items: &[Item], font: &GlyphNames, out: &mut Vec<(String, Range<usize>)>) {
    for item in items {
        let (Kind::GlyphName | Kind::GlyphNameOrRange) = item.kind else {
            continue;
        };
        let name = item.text.unwrap_or_default().trim_start_matches('\\');
        if font.contains(name) || is_font_range(name, font) {
            continue;
        }
        out.push((name.to_string(), item.range.clone()));
    }
}

fn missing_glyph_diagnostics<'f>(
    pieces: impl Iterator<Item = (usize, &'f Facts)>,
    ctx: &LintOptions,
    cache: &mut Suggestions,
    out: &mut Vec<Diagnostic>,
) {
    let Some(font) = ctx.glyphs else { return };
    // each suggestion costs an edit distance against every glyph, so only the
    // first names get one; `cache` keeps them across calls (the incremental
    // linter owns one for its lifetime)
    const MAX_SUGGESTED: usize = 50;
    let mut suggestions: HashMap<String, Option<String>> = HashMap::new();
    for (at, f) in pieces {
        for (name, range) in &f.missing_glyphs {
            let range = range.start + at..range.end + at;
            let mut d = Diagnostic {
                rule: Rule::GlyphNotInFont,
                severity: Severity::Error,
                range: range.clone(),
                message: format!("glyph '{name}' is not in the font"),
                fixes: Vec::new(),
                related: None,
            };
            let suggestion = if suggestions.len() < MAX_SUGGESTED || suggestions.contains_key(name)
            {
                let value = cache
                    .entry(name.clone())
                    .or_insert_with(|| closest(name, font.iter(), 2).map(String::from))
                    .clone();
                suggestions.entry(name.clone()).or_insert(value).clone()
            } else {
                None
            };
            if let Some(best) = suggestion {
                d.message.push_str(&format!("; did you mean '{best}'?"));
                d.fixes.push(Fix::new(
                    format!("Replace with '{best}'"),
                    vec![TextEdit {
                        range,
                        new_text: best,
                    }],
                    Applicability::Unsafe,
                ));
            }
            out.push(d);
        }
    }
}

/// `a-z` style token whose two ends are both glyphs in the font.
fn is_font_range(name: &str, font: &GlyphNames) -> bool {
    name.match_indices('-')
        .any(|(i, _)| font.contains(&name[..i]) && font.contains(&name[i + 1..]))
}

// ---------------------------------------------------------------- class-length-mismatch

fn maybe_range(name: &str, font: Option<&GlyphNames>) -> bool {
    match font {
        Some(font) => !font.contains(name) && is_font_range(name, font),
        None => name
            .split_once('-')
            .is_some_and(|(a, b)| a.len() <= 3 && b.len() <= 3 && !a.is_empty() && !b.is_empty()),
    }
}

/// The glyph names in an FEA range, ends included: names of equal length that
/// differ in one ASCII letter of the same case (`a - z`, `a.sc - z.sc`) or in
/// a run of up to three digits (`x.001 - x.010`). `None` when the two names
/// don't form a valid range.
fn expand_range(start: &str, end: &str) -> Option<Vec<String>> {
    if start.len() != end.len() || !start.is_ascii() || !end.is_ascii() {
        return None;
    }
    let (a, b) = (start.as_bytes(), end.as_bytes());
    let front = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let back = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    if front + back >= a.len() {
        return None;
    }
    let diff = front..a.len() - back;
    let one_letter = diff.len() == 1
        && a[front].is_ascii_alphabetic()
        && b[front].is_ascii_alphabetic()
        && a[front].is_ascii_uppercase() == b[front].is_ascii_uppercase()
        && a[front] < b[front];
    if one_letter {
        return Some(
            (a[front]..=b[front])
                .map(|c| format!("{}{}{}", &start[..front], c as char, &start[diff.end..]))
                .collect(),
        );
    }
    let (from, to) = (
        start[diff.clone()].parse::<u16>().ok()?,
        end[diff.clone()].parse::<u16>().ok()?,
    );
    if diff.len() > 3 || from >= to {
        return None;
    }
    let width = diff.len();
    Some(
        (from..=to)
            .map(|n| format!("{}{n:0width$}{}", &start[..front], &start[diff.end..]))
            .collect(),
    )
}

/// A hyphenated token that is a range rather than one glyph, expanded.
fn token_range(name: &str, font: Option<&GlyphNames>) -> Option<Vec<String>> {
    name.match_indices('-').find_map(|(i, _)| {
        let (a, b) = (&name[..i], &name[i + 1..]);
        let known = font.is_none_or(|f| f.contains(a) && f.contains(b));
        known.then(|| expand_range(a, b)).flatten()
    })
}

/// Members of a class body or rule operand; `None` when one can't be expanded
/// statically (Glyphs predicates, CIDs, malformed ranges).
fn members(items: &[&Item], font: Option<&GlyphNames>) -> Option<Vec<Member>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let item = items[i];
        i += 1;
        match item.kind {
            Kind::GlyphName | Kind::GlyphNameOrRange => {
                let name = item.text?.trim_start_matches('\\');
                // the stand-in glyph map reads `a-z` as one name; expand it if it's a range
                if maybe_range(name, font) {
                    out.extend(token_range(name, font)?.into_iter().map(Member::Glyph));
                    continue;
                }
                out.push(Member::Glyph(name.to_string()));
            }
            Kind::NamedGlyphClass => out.push(Member::Class(item.text?.to_string())),
            Kind::GlyphRange => {
                let ends: Vec<&str> = items[i..]
                    .iter()
                    .take_while(|x| x.range.start < item.range.end)
                    .filter(|x| matches!(x.kind, Kind::GlyphName | Kind::GlyphNameOrRange))
                    .filter_map(|x| x.text)
                    .collect();
                let [start, end] = ends[..] else { return None };
                out.extend(expand_range(start, end)?.into_iter().map(Member::Glyph));
                // skip the range's own tokens
                while i < items.len() && items[i].range.start < item.range.end {
                    i += 1;
                }
            }
            Kind::GlyphsPredicateNode | Kind::Cid => return None,
            _ => {}
        }
    }
    Some(out)
}

/// The item at `at` and everything nested inside it.
fn operand<'b>(items: &'b [Item<'b>], at: usize) -> impl Iterator<Item = &'b Item<'b>> {
    let end = items[at].range.end;
    let depth = items[at].depth;
    items[at..]
        .iter()
        .take_while(move |x| x.range.start < end || (x.range.is_empty() && x.depth > depth))
}

fn class_facts(items: &[Item], source: &str, font: Option<&GlyphNames>, facts: &mut Facts) {
    for (i, item) in items.iter().enumerate() {
        match item.kind {
            Kind::GlyphClassDefNode => {
                let Some(name) = items[i + 1..]
                    .iter()
                    .find(|x| x.kind == Kind::NamedGlyphClass)
                else {
                    continue;
                };
                let body = items[i + 1..]
                    .iter()
                    .take_while(|x| x.range.start < item.range.end);
                let body: Vec<&Item> = body.skip_while(|x| x.kind != Kind::Eq).skip(1).collect();
                let value = members(&body, font);
                facts.class_defs.push((
                    name.text.unwrap_or_default().to_string(),
                    name.range.clone(),
                    value,
                ));
            }
            Kind::GsubType1 => {
                let depth = item.depth + 1;
                let children: Vec<(usize, &Item)> = items[i + 1..]
                    .iter()
                    .enumerate()
                    .take_while(|(_, x)| x.range.start < item.range.end)
                    .filter(|(_, x)| {
                        x.depth == depth && !matches!(x.kind, Kind::Whitespace | Kind::Comment)
                    })
                    .map(|(j, x)| (i + 1 + j, x))
                    .collect();
                let Some(by) = children.iter().position(|(_, x)| x.kind == Kind::ByKw) else {
                    continue;
                };
                let (Some(&(ti, target)), Some(&(ri, replacement))) =
                    (children.get(1), children.get(by + 1))
                else {
                    continue;
                };
                let is_class =
                    |x: &Item| matches!(x.kind, Kind::GlyphClass | Kind::NamedGlyphClass);
                if !is_class(target) || !is_class(replacement) {
                    continue;
                }
                facts.sub_pairs.push(SubPair {
                    range: target.range.start..replacement.range.end,
                    target: members(&operand(items, ti).collect::<Vec<_>>(), font),
                    replacement: members(&operand(items, ri).collect::<Vec<_>>(), font),
                    left: source[target.range.clone()].chars().take(40).collect(),
                    right: source[replacement.range.clone()].chars().take(40).collect(),
                });
            }
            _ => {}
        }
    }
}

fn class_length_mismatch<'f>(
    pieces: impl Iterator<Item = (usize, &'f Facts)> + Clone,
    out: &mut Vec<Diagnostic>,
) {
    // the last definition of a name wins, as in a single pass over the document
    let mut table: HashMap<&str, &Option<Vec<Member>>> = HashMap::new();
    for (_, f) in pieces.clone() {
        for (name, _, value) in &f.class_defs {
            table.insert(name, value);
        }
    }
    fn expand(
        members: &[Member],
        table: &HashMap<&str, &Option<Vec<Member>>>,
        depth: usize,
    ) -> Option<Vec<String>> {
        if depth > 32 {
            return None;
        }
        let mut out = Vec::new();
        for m in members {
            match m {
                Member::Glyph(g) => out.push(g.clone()),
                Member::Class(c) => {
                    out.extend(expand(table.get(c.as_str())?.as_ref()?, table, depth + 1)?)
                }
            }
        }
        Some(out)
    }
    for (at, f) in pieces {
        for pair in &f.sub_pairs {
            let (Some(t), Some(r)) = (&pair.target, &pair.replacement) else {
                continue;
            };
            let (Some(from), Some(to)) = (expand(t, &table, 0), expand(r, &table, 0)) else {
                continue;
            };
            if from.len() == to.len() || to.len() == 1 {
                continue;
            }
            let (left, right) = (&pair.left, &pair.right);
            let range = pair.range.start + at..pair.range.end + at;
            let message = match naming_pattern_hint(&from, &to, left, right) {
                Some(hint) => format!(
                    "{left} has {} glyphs but {right} has {}; {hint}",
                    from.len(),
                    to.len()
                ),
                None => {
                    let paired = from.len().min(to.len());
                    let (longer, extra) = if from.len() > to.len() {
                        ("target", &from[paired..])
                    } else {
                        ("replacement", &to[paired..])
                    };
                    let shown: Vec<&str> = extra.iter().take(6).map(String::as_str).collect();
                    let more = if extra.len() > 6 {
                        format!(" and {} more", extra.len() - 6)
                    } else {
                        String::new()
                    };
                    format!(
                        "{left} has {} glyphs but {right} has {}; unpaired in the {longer}: {}{more}",
                        from.len(),
                        to.len(),
                        shown.join(" ")
                    )
                }
            };
            out.push(Diagnostic {
                rule: Rule::ClassLengthMismatch,
                severity: Severity::Error,
                range,
                message,
                fixes: Vec::new(),
                related: None,
            });
        }
    }
}

// ---------------------------------------------------------------- unknown-feature-tag

const REGISTERED: &str = "aalt abvf abvm abvs afrc akhn apkn blwf blwm blws c2pc c2sc calt case ccmp cfar \
chws cjct clig cpct cpsp cswh curs dist dlig dnom dtls expt falt fin2 fin3 fina flac frac fwid half haln \
halt hist hkna hlig hngl hojo hwid init isol ital jalt jp04 jp78 jp83 jp90 kern lfbd liga ljmo lnum locl \
ltra ltrm mark med2 medi mgrk mkmk mset nalt nlck nukt numr onum opbd ordn ornm palt pcap pkna pnum pref \
pres pstf psts pwid qwid rand rclt rkrf rlig rphf rtbd rtla rtlm ruby rvrn salt sinf size smcp smpl ssty \
stch subs sups swsh titl tjmo tnam tnum trad twid unic valt vapk vatu vchw vert vhal vjmo vkna vkrn vpal \
vrt2 vrtr zero";

fn registered_tags() -> Vec<String> {
    let mut tags: Vec<String> = REGISTERED.split_whitespace().map(String::from).collect();
    tags.extend((1..=99).map(|n| format!("cv{n:02}")));
    tags.extend((1..=20).map(|n| format!("ss{n:02}")));
    tags
}

fn unknown_feature_tag(items: &[Item], out: &mut Vec<Diagnostic>) {
    let tags = registered_tags();
    // feature blocks, and `feature xxxx;` references inside `aalt`
    for (i, item) in items.iter().enumerate() {
        if !matches!(item.kind, Kind::FeatureNode | Kind::AaltFeatureNode) {
            continue;
        }
        let inside = || {
            items[i + 1..]
                .iter()
                .take_while(|x| x.range.start < item.range.end)
        };
        // a name longer than four characters isn't lexed as a tag at all, but as an identifier
        let Some(tag) = inside().find(|x| matches!(x.kind, Kind::Tag | Kind::Ident)) else {
            continue;
        };
        let text = tag.text.unwrap_or_default();
        // tags starting with an uppercase letter are reserved for private use
        let private = text.starts_with(|c: char| c.is_ascii_uppercase()) && text.len() <= 4;
        if private || tags.iter().any(|t| t == text) {
            continue;
        }
        let what = if text.len() > 4 {
            "is not a valid tag (tags have at most four characters)"
        } else {
            "is not a registered OpenType feature tag"
        };
        let mut d = Diagnostic {
            rule: Rule::UnknownFeatureTag,
            severity: if text.len() > 4 {
                Severity::Error
            } else {
                Severity::Warning
            },
            range: tag.range.clone(),
            message: format!("'{text}' {what}"),
            fixes: Vec::new(),
            related: None,
        };
        let candidates = closest_all(text, tags.iter().map(String::as_str), 2);
        if !candidates.is_empty() {
            let quoted: Vec<String> = candidates.iter().map(|c| format!("'{c}'")).collect();
            d.message
                .push_str(&format!("; did you mean {}?", quoted.join(" or ")));
        }
        for best in candidates {
            // rename the closing tag too
            let edits = inside()
                .filter(|x| x.kind == tag.kind && x.depth == tag.depth && x.text == Some(text))
                .map(|x| TextEdit {
                    range: x.range.clone(),
                    new_text: best.to_string(),
                })
                .collect();
            d.fixes.push(Fix::new(
                format!("Rename feature to '{best}'"),
                edits,
                Applicability::Unsafe,
            ));
        }
        out.push(d);
    }
}

// ---------------------------------------------------------------- duplicate-glyph-in-class

/// Byte ranges on the replacement side of substitutions (`by …` / `from …`),
/// where repeating a glyph is meaningful: `sub [Alphatonos Alpha] by [Alpha Alpha];`.
fn replacement_ranges(items: &[Item]) -> Vec<Range<usize>> {
    // items are in source order and these ranges never nest, so the list is sorted
    items
        .iter()
        .filter(|x| matches!(x.kind, Kind::ByKw | Kind::FromKw) && x.parent != usize::MAX)
        .map(|x| x.range.start..items[x.parent].range.end)
        .collect()
}

fn within_any(sorted: &[Range<usize>], r: &Range<usize>) -> bool {
    let i = sorted.partition_point(|x| x.start <= r.start);
    i > 0 && sorted[i - 1].end >= r.end
}

fn duplicate_candidates(items: &[Item], facts: &mut Facts) {
    let replacements = replacement_ranges(items);
    let in_replacement = |r: &Range<usize>| within_any(&replacements, r);
    facts.replacement_classes.extend(
        items
            .iter()
            .filter(|x| x.kind == Kind::NamedGlyphClass && in_replacement(&x.range))
            .filter_map(|x| x.text.map(String::from)),
    );

    for (i, class) in items.iter().enumerate() {
        if class.kind != Kind::GlyphClass || in_replacement(&class.range) {
            continue;
        }
        let defined_name = (class.parent != usize::MAX
            && items[class.parent].kind == Kind::GlyphClassDefNode)
            .then(|| {
                items[class.parent + 1..]
                    .iter()
                    .find(|x| x.kind == Kind::NamedGlyphClass)
                    .and_then(|x| x.text)
            })
            .flatten();
        let depth = class.depth + 1;
        let mut seen: HashMap<&str, Range<usize>> = HashMap::new();
        let members = items[i + 1..]
            .iter()
            .take_while(|x| x.range.start < class.range.end)
            .filter(|x| x.depth == depth);
        let mut prev_end = class.range.start + 1;
        for m in members {
            if !matches!(m.kind, Kind::GlyphName | Kind::GlyphNameOrRange) {
                if !matches!(m.kind, Kind::Whitespace | Kind::Comment) {
                    prev_end = m.range.end;
                }
                continue;
            }
            let name = m.text.unwrap_or_default().trim_start_matches('\\');
            match seen.get(name) {
                Some(first) => {
                    let d = Diagnostic {
                        rule: Rule::DuplicateGlyphInClass,
                        severity: Severity::Warning,
                        range: m.range.clone(),
                        message: format!("'{name}' is already in this class"),
                        fixes: vec![Fix::new(
                            format!("Remove the second '{name}'"),
                            vec![TextEdit {
                                range: prev_end..m.range.end,
                                new_text: String::new(),
                            }],
                            Applicability::Safe,
                        )],
                        related: Some(Related {
                            range: first.clone(),
                            label: "first occurrence".into(),
                        }),
                    };
                    facts.duplicates.push((defined_name.map(String::from), d));
                }
                None => {
                    seen.insert(name, m.range.clone());
                }
            }
            prev_end = m.range.end;
        }
    }
}

// ---------------------------------------------------------------- invisible-character

fn invisible_characters(items: &[Item], source: &str, out: &mut Vec<Diagnostic>) {
    // characters inside comments and strings are the author's business
    let skip: Vec<Range<usize>> = items
        .iter()
        .filter(|x| matches!(x.kind, Kind::Comment | Kind::String))
        .map(|x| x.range.clone())
        .collect();
    for (at, c) in source.char_indices() {
        let replacement = match c {
            '\u{00A0}' | '\u{2007}' | '\u{202F}' | '\u{3000}' => Some(" "),
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{00AD}' => Some(""),
            '\u{FEFF}' if at > 0 => Some(""),
            c if !c.is_ascii() => None,
            _ => continue,
        };
        if skip.iter().any(|r| r.contains(&at)) {
            continue;
        }
        let range = at..at + c.len_utf8();
        let fixes = replacement
            .map(|r| {
                let title = if r.is_empty() {
                    "Delete the character".to_string()
                } else {
                    "Replace with a space".to_string()
                };
                let edit = TextEdit {
                    range: range.clone(),
                    new_text: r.to_string(),
                };
                vec![Fix::new(title, vec![edit], Applicability::Safe)]
            })
            .unwrap_or_default();
        out.push(Diagnostic {
            rule: Rule::InvisibleCharacter,
            severity: Severity::Error,
            range,
            message: format!(
                "{} (U+{:04X}) outside a comment or string",
                char_name(c),
                c as u32
            ),
            fixes,
            related: None,
        });
    }
}

fn char_name(c: char) -> &'static str {
    match c {
        '\u{00A0}' => "non-breaking space",
        '\u{2007}' => "figure space",
        '\u{202F}' => "narrow no-break space",
        '\u{3000}' => "ideographic space",
        '\u{200B}' => "zero-width space",
        '\u{200C}' => "zero-width non-joiner",
        '\u{200D}' => "zero-width joiner",
        '\u{2060}' => "word joiner",
        '\u{00AD}' => "soft hyphen",
        '\u{FEFF}' => "byte-order mark",
        _ => "non-ASCII character",
    }
}

// ---------------------------------------------------------------- unused / undefined classes and lookups

fn definition_facts(items: &[Item], facts: &mut Facts) {
    for (i, item) in items.iter().enumerate() {
        let name = item.text.unwrap_or_default().to_string();
        match item.kind {
            Kind::NamedGlyphClass if item.parent_kind == Kind::MarkClassNode => {
                facts.mark_classes.push((name, item.range.clone()))
            }
            Kind::NamedGlyphClass => {
                let defining = item.parent_kind == Kind::GlyphClassDefNode
                    && items[..i]
                        .iter()
                        .rev()
                        .take_while(|x| x.kind != Kind::GlyphClassDefNode)
                        .all(|x| x.kind != Kind::NamedGlyphClass);
                if !defining {
                    facts.class_uses.push((name, item.range.clone()));
                }
            }
            Kind::Label if item.parent_kind == Kind::LookupBlockNode => {
                // a lookup defined inside a feature block runs where it's defined, so it is used
                let in_feature = has_ancestor(items, item, Kind::FeatureNode);
                facts
                    .lookup_defs
                    .push((name, item.range.clone(), in_feature));
            }
            Kind::Ident if item.parent_kind == Kind::LookupRefNode => {
                facts.lookup_uses.push((name, item.range.clone()))
            }
            _ => {}
        }
    }
}

fn definitions<'f>(
    pieces: impl Iterator<Item = (usize, &'f Facts)> + Clone,
    project: bool,
    out: &mut Vec<Diagnostic>,
) {
    let at = |a: usize, r: &Range<usize>| r.start + a..r.end + a;
    let mut class_defs: BTreeMap<&str, Range<usize>> = BTreeMap::new();
    let mut mark_classes: HashMap<&str, Range<usize>> = HashMap::new();
    let mut lookup_defs: BTreeMap<&str, Range<usize>> = BTreeMap::new();
    let mut in_feature: HashSet<&str> = HashSet::new();
    let mut class_uses: Vec<(&str, Range<usize>)> = Vec::new();
    let mut lookup_uses: Vec<(&str, Range<usize>)> = Vec::new();
    for (a, f) in pieces {
        for (n, r, _) in &f.class_defs {
            class_defs.entry(n).or_insert(at(a, r));
        }
        for (n, r) in &f.mark_classes {
            mark_classes.entry(n).or_insert(at(a, r));
        }
        for (n, r, inside) in &f.lookup_defs {
            lookup_defs.entry(n).or_insert(at(a, r));
            if *inside {
                in_feature.insert(n);
            }
        }
        class_uses.extend(f.class_uses.iter().map(|(n, r)| (n.as_str(), at(a, r))));
        lookup_uses.extend(f.lookup_uses.iter().map(|(n, r)| (n.as_str(), at(a, r))));
    }

    let used_classes: HashSet<&str> = class_uses.iter().map(|(n, _)| *n).collect();
    let used_lookups: HashSet<&str> = lookup_uses.iter().map(|(n, _)| *n).collect();
    let scope = if project {
        "in the project"
    } else {
        "in this file"
    };

    for (name, range) in &class_defs {
        if !used_classes.contains(name) {
            out.push(Diagnostic {
                rule: Rule::UnusedClass,
                severity: Severity::Hint,
                range: range.clone(),
                message: format!("class {name} is defined but never used {scope}"),
                fixes: Vec::new(),
                related: None,
            });
        }
    }
    for (name, range) in &lookup_defs {
        if !used_lookups.contains(name) && !in_feature.contains(name) {
            out.push(Diagnostic {
                rule: Rule::UnusedLookup,
                severity: Severity::Hint,
                range: range.clone(),
                message: format!(
                    "standalone lookup {name} is never referenced {scope}, so it never runs"
                ),
                fixes: Vec::new(),
                related: None,
            });
        }
    }

    // only a whole project can say a name is missing; a single file may rely on others
    if !project {
        return;
    }
    let mut reported: HashSet<&str> = HashSet::new();
    for (name, range) in &class_uses {
        let def = class_defs.get(name).or_else(|| mark_classes.get(name));
        let problem = match def {
            None => format!("class {name} is never defined in the project"),
            Some(d) if d.start > range.start => {
                format!("class {name} is used before it is defined")
            }
            Some(_) => continue,
        };
        if !reported.insert(name) {
            continue;
        }
        out.push(Diagnostic {
            rule: Rule::UndefinedClass,
            severity: Severity::Error,
            range: range.clone(),
            message: problem,
            fixes: Vec::new(),
            related: None,
        });
    }
    for (name, range) in &lookup_uses {
        let problem = match lookup_defs.get(name) {
            None => format!("lookup {name} is never defined in the project"),
            Some(d) if d.start > range.start => {
                format!("lookup {name} is used before it is defined")
            }
            Some(_) => continue,
        };
        out.push(Diagnostic {
            rule: Rule::UndefinedLookup,
            severity: Severity::Error,
            range: range.clone(),
            message: problem,
            fixes: Vec::new(),
            related: None,
        });
    }
}

// ---------------------------------------------------------------- helpers

/// Closest candidate within `max` edits (ties: first). Skips exact matches.
fn closest<'c>(
    word: &str,
    candidates: impl Iterator<Item = &'c str>,
    max: usize,
) -> Option<&'c str> {
    closest_all(word, candidates, max).into_iter().next()
}

/// Every candidate at the smallest edit distance (at most three of them).
fn closest_all<'c>(
    word: &str,
    candidates: impl Iterator<Item = &'c str>,
    max: usize,
) -> Vec<&'c str> {
    let scored: Vec<(usize, &str)> = candidates
        .filter(|c| c.len().abs_diff(word.len()) <= max)
        .map(|c| (edit_distance(word, c), c))
        .filter(|(d, _)| *d > 0 && *d <= max)
        .collect();
    let Some(best) = scored.iter().map(|(d, _)| *d).min() else {
        return Vec::new();
    };
    scored
        .into_iter()
        .filter(|(d, _)| *d == best)
        .map(|(_, c)| c)
        .take(3)
        .collect()
}

/// Edit distance where swapping two adjacent characters counts as one edit
/// (optimal string alignment), so `smpc` is one edit from `smcp`.
#[allow(clippy::needless_range_loop)] // the textbook DP reads clearest with indices
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=b.len() {
        d[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

/// A rule that explains a parse error better (and fixes it) replaces fea-rs's message.
fn drop_covered_syntax_errors(out: &mut Vec<Diagnostic>) {
    const SPECIFIC: [Rule; 3] = [
        Rule::InvisibleCharacter,
        Rule::GlyphNotInFont,
        Rule::UnknownFeatureTag,
    ];
    let covered: Vec<Range<usize>> = out
        .iter()
        .filter(|d| SPECIFIC.contains(&d.rule))
        .flat_map(|d| {
            std::iter::once(d.range.clone()).chain(
                d.fixes
                    .iter()
                    .flat_map(|f| f.edits.iter().map(|e| e.range.clone())),
            )
        })
        .collect();
    let overlaps = |r: &Range<usize>| covered.iter().any(|c| c.start < r.end && r.start < c.end);
    out.retain(|d| d.rule != Rule::Syntax || !overlaps(&d.range));
}

/// When the replacement names follow the targets plus a suffix (`four` → `four.smcp`),
/// name the glyphs that break the pattern instead of the unpaired tail.
fn naming_pattern_hint(from: &[String], to: &[String], left: &str, right: &str) -> Option<String> {
    let suffix = to.first()?.strip_prefix(from.first()?.as_str())?;
    if suffix.is_empty() {
        return None;
    }
    let expected: Vec<String> = from.iter().map(|g| format!("{g}{suffix}")).collect();
    let have: HashSet<&str> = to.iter().map(String::as_str).collect();
    let wanted: HashSet<&str> = expected.iter().map(String::as_str).collect();
    let missing: Vec<&str> = expected
        .iter()
        .map(String::as_str)
        .filter(|g| !have.contains(g))
        .collect();
    let extra: Vec<&str> = to
        .iter()
        .map(String::as_str)
        .filter(|g| !wanted.contains(g))
        .collect();
    let list = |v: &[&str]| {
        let more = if v.len() > 5 {
            format!(" and {} more", v.len() - 5)
        } else {
            String::new()
        };
        format!(
            "{}{more}",
            v.iter().take(5).copied().collect::<Vec<_>>().join(" ")
        )
    };
    match (missing.is_empty(), extra.is_empty()) {
        (false, true) => Some(format!("{right} is missing {}", list(&missing))),
        (true, false) => Some(format!("{right} has extra {}", list(&extra))),
        (false, false) => Some(format!(
            "{right} is missing {}; it also has {} which {left} doesn't pair",
            list(&missing),
            list(&extra)
        )),
        (true, true) => None,
    }
}

fn has_ancestor(items: &[Item], item: &Item, kind: Kind) -> bool {
    let mut at = item.parent;
    while at != usize::MAX {
        if items[at].kind == kind {
            return true;
        }
        at = items[at].parent;
    }
    false
}

#[cfg(test)]
mod range_tests {
    use super::expand_range;

    #[test]
    fn letter_ranges_include_both_ends() {
        assert_eq!(expand_range("a", "d").unwrap(), ["a", "b", "c", "d"]);
        assert_eq!(
            expand_range("A.sc", "C.sc").unwrap(),
            ["A.sc", "B.sc", "C.sc"]
        );
    }

    #[test]
    fn number_ranges_keep_their_width_and_include_the_end() {
        assert_eq!(
            expand_range("x.001", "x.003").unwrap(),
            ["x.001", "x.002", "x.003"]
        );
        assert_eq!(
            expand_range("x.09", "x.11").unwrap(),
            ["x.09", "x.10", "x.11"]
        );
    }

    #[test]
    fn rejects_names_that_are_not_a_range() {
        assert_eq!(expand_range("a", "Z"), None);
        assert_eq!(expand_range("ab", "cd"), None);
        assert_eq!(expand_range("d", "a"), None);
        assert_eq!(expand_range("x.1", "x.10"), None);
    }
}
