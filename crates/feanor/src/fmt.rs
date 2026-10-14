//! The formatter: layout only, never a token.
//!
//! [`format()`] rewrites spacing, indentation, line breaks and blank lines, and
//! nothing else: every token and comment comes out with the same text, in the
//! same order. Top-level items that don't parse are left exactly as written.
//!
//! How it works: fea-rs's tree is lossless but attaches whitespace
//! inconsistently (sometimes inside a node, sometimes between nodes), so the
//! printer works on a flat stream of tokens and comments and uses the tree only
//! for hints: glyph-class boundaries, the node a token belongs to, and which
//! top-level items contain parse errors.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

#[path = "align.rs"]
mod align;

use fea_rs::{GlyphMap, Kind, Node, NodeOrToken, ParseTree};

/// The indent unit for nested blocks and class contents.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Indent {
    /// This many spaces per level.
    Spaces(usize),
    /// One tab per level.
    Tab,
    /// The unit the text already uses (see [`detect_indent`]); four spaces if
    /// nothing is indented yet. Keeps the diff small when a project adopts the formatter.
    Auto,
}

/// How [`format()`] lays code out.
///
/// ```
/// use feanor::{format, FormatOptions, Indent};
///
/// let mut options = FormatOptions::default();
/// options.indent = Indent::Spaces(2);
/// let out = format("feature liga {\nsub f i by f_i;\n} liga;\n", &options);
/// assert_eq!(out.text, "feature liga {\n  sub f i by f_i;\n} liga;\n");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FormatOptions {
    /// The width that glyph classes are packed to and long rules wrapped at, in
    /// characters. Lines can still run past it: an unbreakable token, a long
    /// comment, or a hand-aligned class table.
    pub line_width: usize,
    /// The indent unit; defaults to [`Indent::Auto`].
    pub indent: Indent,
    /// Keep runs of statements the author aligned by hand aligned, with the
    /// columns recomputed (as gofmt does). When false, they're single-spaced.
    pub keep_alignment: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions {
            line_width: 100,
            indent: Indent::Auto,
            keep_alignment: true,
        }
    }
}

/// The result of [`format()`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Formatted {
    /// The formatted text, ending in a line break (unless the input ended
    /// inside an untouched region without one).
    pub text: String,
    /// Byte ranges of the input left exactly as written: top-level items that
    /// don't parse, and `# fmt: off` regions.
    pub untouched: Vec<Range<usize>>,
    /// The input was formatted as a feature body (the inside of a `feature`
    /// block), because it only parses that way.
    pub feature_body: bool,
}

/// Format feature code.
///
/// `source` can be a whole feature file or a feature body: when the text only
/// parses wrapped in a `feature` block, it's formatted that way and returned
/// without the wrapper. Line endings are kept: a file that mostly uses `\r\n`
/// comes back with `\r\n` throughout. The output is idempotent: formatting it
/// again returns it unchanged.
///
/// ```
/// use feanor::{format, FormatOptions};
///
/// let out = format("@lc=[a   b c];\nfeature liga {sub f i by f_i;} liga;\n", &FormatOptions::default());
/// assert_eq!(out.text, "@lc = [a b c];\nfeature liga { sub f i by f_i; } liga;\n");
/// ```
pub fn format(source: &str, options: &FormatOptions) -> Formatted {
    // a broken `include` can't be parsed safely (see `bare_includes`): leave the text as written
    if !bare_includes(source).is_empty() {
        return Formatted {
            text: source.to_string(),
            // one range: the whole text
            untouched: std::iter::once(0..source.len()).collect(),
            feature_body: false,
        };
    }
    // keep the file's line endings: format with `\n`, then put `\r\n` back
    if uses_crlf(source) {
        let lf = source.replace("\r\n", "\n");
        let mut out = format_lf(&lf, options);
        out.text = out.text.replace("\n", "\r\n");
        let to_source = CrlfOffsets::new(source);
        out.untouched = out
            .untouched
            .iter()
            .map(|r| to_source.map(r.start)..to_source.map(r.end))
            .collect();
        return out;
    }
    format_lf(source, options)
}

/// Whether most line breaks in `text` are `\r\n`.
fn uses_crlf(text: &str) -> bool {
    let crlf = text.matches("\r\n").count();
    crlf > 0 && crlf * 2 >= text.matches('\n').count()
}

/// Maps offsets in a text with `\r\n` replaced by `\n` back to the original.
struct CrlfOffsets {
    /// Offsets (in the `\n` text) of each `\n` that was a `\r\n`.
    removed: Vec<usize>,
}

impl CrlfOffsets {
    fn new(original: &str) -> Self {
        let removed = original
            .match_indices("\r\n")
            .enumerate()
            .map(|(i, (at, _))| at - i)
            .collect();
        CrlfOffsets { removed }
    }

    fn map(&self, lf_offset: usize) -> usize {
        lf_offset + self.removed.partition_point(|&at| at < lf_offset)
    }
}

fn format_lf(source: &str, options: &FormatOptions) -> Formatted {
    let indent = match &options.indent {
        Indent::Spaces(n) => " ".repeat(*n),
        Indent::Tab => "\t".into(),
        Indent::Auto => detect_indent(source).unwrap_or_else(|| "    ".into()),
    };
    let settings = Settings {
        width: options.line_width,
        indent,
        keep_alignment: options.keep_alignment,
    };
    let as_file = format_file(source, &settings);
    if as_file.untouched.is_empty() {
        return as_file;
    }
    match format_body(source, &settings) {
        Some(as_body) if as_body.untouched.len() < as_file.untouched.len() => as_body,
        _ => as_file,
    }
}

/// [`FormatOptions`] resolved for one text.
#[derive(Clone, Debug)]
pub(crate) struct Settings {
    width: usize,
    indent: String,
    keep_alignment: bool,
}

#[derive(Clone, Debug)]
struct Tok<'a> {
    text: &'a str,
    kind: Kind,
    parent: Kind,
    ws_before: bool,
    nl_before: usize,
    /// Width of the whitespace before this token on its own line (alignment padding).
    ws_len: usize,
    /// (class literal id, unit index) when inside a glyph class literal's brackets.
    class: Option<(u32, u32)>,
    open: Option<u32>,
    close: Option<u32>,
}

#[derive(Clone, Debug)]
enum Elem<'a> {
    Tok(Tok<'a>),
    Comment {
        text: &'a str,
        nl_before: usize,
        class: Option<(u32, u32)>,
    },
    Verbatim {
        text: String,
        nl_before: usize,
    },
}

impl Elem<'_> {
    fn nl_before(&self) -> usize {
        match self {
            Elem::Tok(t) => t.nl_before,
            Elem::Comment { nl_before, .. } | Elem::Verbatim { nl_before, .. } => *nl_before,
        }
    }
}

struct Flattener<'a> {
    src: &'a str,
    /// Byte offset of the next token; tracked here because fea-rs only fills
    /// in token positions for multi-file parses.
    pos: usize,
    elems: Vec<Elem<'a>>,
    ws: bool,
    nl: usize,
    ws_len: usize,
    next_class: u32,
    skipped: Vec<std::ops::Range<usize>>,
    off: Option<OffRegion<'a>>,
}

/// An open `# fmt: off` region: elements are buffered until `# fmt: on`.
struct OffRegion<'a> {
    start: usize,
    nl_before: usize,
    braces: i32,
    buffered: Vec<Elem<'a>>,
}

fn directive(comment: &str) -> Option<bool> {
    let body = comment.trim_start_matches('#').trim();
    match body {
        "fmt: off" => Some(false),
        "fmt: on" => Some(true),
        _ => None,
    }
}

/// The kind the layout treats a token as. Braces in a Glyphs number expression
/// (`${x.afrc:width*4.5}`) delimit an expression, not a block: they're laid out
/// like parentheses, with no space inside and no line breaks.
fn layout_kind(kind: Kind, parent: Kind) -> Kind {
    match (kind, parent) {
        (Kind::LBrace, Kind::GlyphsNumberValueExprNode) => Kind::LParen,
        (Kind::RBrace, Kind::GlyphsNumberValueExprNode) => Kind::RParen,
        _ => kind,
    }
}

fn has_error(node: &Node) -> bool {
    node.error
        || node
            .iter_children()
            .any(|c| c.as_node().is_some_and(has_error))
}

impl<'a> Flattener<'a> {
    fn trivia(&mut self, text: &str) {
        self.pos += text.len();
        self.ws = true;
        self.nl += text.bytes().filter(|b| *b == b'\n').count();
        match text.rfind('\n') {
            Some(i) => self.ws_len = text.len() - i - 1,
            None => self.ws_len += text.len(),
        }
    }

    fn push(&mut self, elem: Elem<'a>) {
        self.ws = false;
        self.nl = 0;
        self.ws_len = 0;
        let Some(off) = &mut self.off else {
            self.elems.push(elem);
            return;
        };
        if let Elem::Tok(t) = &elem {
            match t.kind {
                Kind::LBrace => off.braces += 1,
                Kind::RBrace => off.braces -= 1,
                _ => {}
            }
        }
        off.buffered.push(elem);
    }

    fn comment(&mut self, text: &'a str, class: Option<(u32, u32)>) {
        let start = self.pos;
        self.pos += text.len();
        let nl_before = self.nl;
        match (directive(text), self.off.is_some()) {
            (Some(false), false) => {
                self.off = Some(OffRegion {
                    start,
                    nl_before,
                    braces: 0,
                    buffered: Vec::new(),
                });
                self.ws = false;
                self.nl = 0;
            }
            (Some(true), true) => {
                self.push(Elem::Comment {
                    text,
                    nl_before,
                    class,
                });
                self.close_off();
            }
            _ => self.push(Elem::Comment {
                text,
                nl_before,
                class,
            }),
        }
    }

    /// End an off region: verbatim if its braces balance, otherwise format it normally.
    fn close_off(&mut self) {
        let Some(off) = self.off.take() else { return };
        if off.braces == 0 {
            let text = self.src[off.start..self.pos].trim_end().to_string();
            self.elems.push(Elem::Verbatim {
                text,
                nl_before: off.nl_before,
            });
            return;
        }
        let first = self.src[off.start..]
            .lines()
            .next()
            .unwrap_or("")
            .trim_end();
        let first: &'a str = &self.src[off.start..off.start + first.len()];
        self.elems.push(Elem::Comment {
            text: first,
            nl_before: off.nl_before,
            class: None,
        });
        self.elems.extend(off.buffered);
    }

    fn walk(&mut self, node: &'a Node, class: Option<(u32, u32)>) {
        let is_class = node.kind() == Kind::GlyphClass;
        let id = if is_class {
            self.next_class += 1;
            Some(self.next_class)
        } else {
            None
        };
        let mut unit = 0u32;
        let children: Vec<_> = node.iter_children().collect();
        let last = children.len().saturating_sub(1);

        for (i, child) in children.into_iter().enumerate() {
            let inner = match id {
                Some(c) if i != 0 && i != last => Some((c, unit)),
                Some(_) => class,
                None => class,
            };

            match child {
                NodeOrToken::Node(n) => {
                    self.walk(n, inner);
                    if id.is_some() {
                        unit += 1;
                    }
                }
                NodeOrToken::Token(t) => match t.kind {
                    Kind::Whitespace => self.trivia(&t.text),
                    Kind::Comment => self.comment(t.text.as_str(), inner),
                    Kind::Eof => {}
                    kind => {
                        self.pos += t.text.len();
                        let tok = Tok {
                            text: t.text.as_str(),
                            kind: layout_kind(kind, node.kind()),
                            parent: node.kind(),
                            ws_before: self.ws,
                            nl_before: self.nl,
                            ws_len: self.ws_len,
                            class: if id.is_some() && (i == 0 || i == last) {
                                class
                            } else {
                                inner
                            },
                            open: id.filter(|_| i == 0),
                            close: id.filter(|_| i == last),
                        };
                        self.push(Elem::Tok(tok));
                        // a backslash escape is part of the following glyph name: keep it glued
                        if id.is_some() && i != 0 && i != last && kind != Kind::Backslash {
                            unit += 1;
                        }
                    }
                },
            }
        }
    }

    fn walk_root(&mut self, root: &'a Node) {
        let children: Vec<_> = root.iter_children().collect();
        let mut i = 0;
        while i < children.len() {
            if is_broken(children[i]) {
                i = self.verbatim_run(&children, i);
                continue;
            }
            match children[i] {
                NodeOrToken::Node(n) => self.walk(n, None),
                NodeOrToken::Token(t) => match t.kind {
                    Kind::Whitespace => self.trivia(&t.text),
                    Kind::Comment => self.comment(t.text.as_str(), None),
                    _ => {}
                },
            }
            i += 1;
        }
    }

    /// Emit a run of top-level items that failed to parse (plus the trivia
    /// between them) exactly as written; returns the index after the run.
    fn verbatim_run(&mut self, children: &[&'a NodeOrToken], start: usize) -> usize {
        let mut end = start;
        for (j, child) in children.iter().enumerate().skip(start + 1) {
            if is_broken(child) {
                end = j;
                continue;
            }
            if !matches!(child.kind(), Kind::Whitespace | Kind::Comment) {
                break;
            }
        }
        let len: usize = children[start..=end].iter().map(|c| c.text_len()).sum();
        let text = self.src[self.pos..self.pos + len].to_string();
        // error recovery can leave an empty broken node (at the end of the
        // file): there's nothing to keep, so it's whitespace, not a region
        if text.trim().is_empty() {
            self.trivia(&text);
            return end + 1;
        }
        self.skipped.push(self.pos..self.pos + len);
        self.pos += len;
        let nl_before = self.nl;
        self.push(Elem::Verbatim { text, nl_before });
        end + 1
    }
}

fn is_broken(child: &NodeOrToken) -> bool {
    match child {
        NodeOrToken::Node(n) => has_error(n),
        NodeOrToken::Token(t) => !matches!(t.kind, Kind::Whitespace | Kind::Comment | Kind::Eof),
    }
}

#[derive(Clone, Debug)]
enum Stmt<'a> {
    Code {
        elems: Vec<Elem<'a>>,
        trailing: Option<&'a str>,
    },
    Comment(&'a str),
    Verbatim(String),
}

#[derive(Clone)]
struct Placed<'a> {
    stmt: Stmt<'a>,
    depth: usize,
    nl_before: usize,
}

impl Stmt<'_> {
    fn opens_block(&self) -> bool {
        matches!(self, Stmt::Code { elems, .. } if matches!(elems.last(), Some(Elem::Tok(t)) if t.kind == Kind::LBrace))
    }

    fn closes_block(&self) -> bool {
        matches!(self, Stmt::Code { elems, .. } if matches!(elems.first(), Some(Elem::Tok(t)) if t.kind == Kind::RBrace))
    }
}

fn group(elems: Vec<Elem<'_>>) -> Vec<Placed<'_>> {
    let mut out: Vec<Placed> = Vec::new();
    let mut cur: Vec<Elem> = Vec::new();
    let mut depth = 0usize;
    let mut cur_depth = 0usize;

    fn flush<'a>(out: &mut Vec<Placed<'a>>, cur: &mut Vec<Elem<'a>>, depth: usize) {
        if cur.is_empty() {
            return;
        }
        let nl_before = cur[0].nl_before();
        out.push(Placed {
            stmt: Stmt::Code {
                elems: std::mem::take(cur),
                trailing: None,
            },
            depth,
            nl_before,
        });
    }

    for elem in elems {
        match elem {
            Elem::Comment {
                text, nl_before, ..
            } if cur.is_empty() => {
                let attach = nl_before == 0 && !out.is_empty();
                if attach {
                    match &mut out.last_mut().unwrap().stmt {
                        Stmt::Code {
                            trailing: t @ None, ..
                        } => {
                            *t = Some(text);
                            continue;
                        }
                        Stmt::Verbatim(v) => {
                            v.push(' ');
                            v.push_str(text);
                            continue;
                        }
                        _ => {}
                    }
                }
                out.push(Placed {
                    stmt: Stmt::Comment(text),
                    depth,
                    nl_before,
                });
            }
            Elem::Verbatim { text, nl_before } => {
                flush(&mut out, &mut cur, cur_depth);
                out.push(Placed {
                    stmt: Stmt::Verbatim(text),
                    depth,
                    nl_before,
                });
            }
            Elem::Tok(ref t) if t.kind == Kind::RBrace => {
                flush(&mut out, &mut cur, cur_depth);
                depth = depth.saturating_sub(1);
                cur_depth = depth;
                cur.push(elem);
            }
            Elem::Tok(ref t) if matches!(t.kind, Kind::Semi | Kind::LBrace) => {
                let opens = t.kind == Kind::LBrace;
                if cur.is_empty() {
                    cur_depth = depth;
                }
                cur.push(elem);
                flush(&mut out, &mut cur, cur_depth);
                if opens {
                    depth += 1;
                }
            }
            elem => {
                if cur.is_empty() {
                    cur_depth = depth;
                }
                cur.push(elem);
            }
        }
    }
    flush(&mut out, &mut cur, cur_depth);
    out
}

fn blank_lines(stmts: &[Placed]) -> Vec<bool> {
    let mut blank = vec![false; stmts.len()];

    for i in 1..stmts.len() {
        let prev = &stmts[i - 1];
        let cur = &stmts[i];
        blank[i] = cur.nl_before >= 2;

        if prev.stmt.opens_block() || cur.stmt.closes_block() {
            blank[i] = false;
        }

        let prev_top_block =
            prev.depth == 0 && (prev.stmt.closes_block() || matches!(prev.stmt, Stmt::Verbatim(_)));
        if prev_top_block {
            blank[i] = true;
        }
    }

    // a top-level block (or verbatim item) gets a blank line before it, placed
    // above any comments directly attached to it
    for i in 1..stmts.len() {
        let top_item = stmts[i].depth == 0
            && (stmts[i].stmt.opens_block() || matches!(stmts[i].stmt, Stmt::Verbatim(_)));
        if !top_item {
            continue;
        }
        let mut j = i;
        while j > 0 && matches!(stmts[j - 1].stmt, Stmt::Comment(_)) && stmts[j].nl_before <= 1 {
            j -= 1;
        }
        if j > 0 {
            blank[j] = true;
        }
    }
    blank
}

fn sep(prev: &Tok, tok: &Tok) -> &'static str {
    match tok.kind {
        Kind::Semi | Kind::Comma | Kind::RSquare | Kind::RAngle | Kind::RParen => return "",
        _ => {}
    }
    // only reachable inside a block kept on one line
    if prev.kind == Kind::Semi || prev.kind == Kind::LBrace || tok.kind == Kind::RBrace {
        return " ";
    }
    match prev.kind {
        Kind::LSquare | Kind::LAngle | Kind::LParen | Kind::Backslash | Kind::Dollar => return "",
        Kind::RBrace => return " ",
        _ => {}
    }
    let class_def_eq = |t: &Tok| t.kind == Kind::Eq && t.parent == Kind::GlyphClassDefNode;
    if class_def_eq(prev) || class_def_eq(tok) {
        return " ";
    }
    if tok.ws_before { " " } else { "" }
}

struct Layout<'o> {
    opts: &'o Settings,
    lines: Vec<String>,
    cur: String,
    /// true when `cur` holds only indentation
    fresh: bool,
}

impl Layout<'_> {
    fn start(&mut self, indent: &str) {
        self.cur = indent.to_string();
        self.fresh = true;
    }

    fn flush(&mut self, next_indent: &str) {
        if !self.fresh {
            self.lines.push(std::mem::take(&mut self.cur));
        }
        self.start(next_indent);
    }

    fn put(&mut self, sep: &str, text: &str) {
        if !self.fresh {
            self.cur.push_str(sep);
        }
        self.cur.push_str(text);
        self.fresh = false;
    }

    fn width(&self) -> usize {
        self.cur.chars().count()
    }

    fn trailing_comment(&mut self, text: &str, next_indent: &str) {
        if self.fresh
            && let Some(last) = self.lines.last_mut()
        {
            last.push(' ');
            last.push_str(text);
            return;
        }
        self.put(" ", text);
        self.flush(next_indent);
    }
}

fn render_flat(toks: &[&Tok]) -> String {
    let mut s = String::new();
    for (i, t) in toks.iter().enumerate() {
        if i > 0 {
            s.push_str(sep(toks[i - 1], t));
        }
        s.push_str(t.text);
    }
    s
}

/// Lay out one statement, breaking the glyph class literals in `broken` into blocks.
fn layout_stmt(
    elems: &[Elem],
    depth: usize,
    broken: &HashSet<u32>,
    kept: &HashSet<u32>,
    items: &Items,
    break_by: bool,
    opts: &Settings,
) -> Vec<String> {
    let base = opts.indent.repeat(depth);
    let inner = opts.indent.repeat(depth + 1);
    let mut lay = Layout {
        opts,
        lines: Vec::new(),
        cur: String::new(),
        fresh: true,
    };
    lay.start(&base);
    let mut prev: Option<&Tok> = None;
    let mut i = 0;

    while i < elems.len() {
        if let Some(&len) = items.at.get(&i)
            && (!items.pack || lay.width() + 1 + len > opts.width)
        {
            lay.flush(&inner);
        }
        match &elems[i] {
            Elem::Comment {
                text, nl_before, ..
            } => {
                if *nl_before == 0 {
                    lay.trailing_comment(text, &inner);
                } else {
                    lay.flush(&inner);
                    lay.put("", text);
                    lay.flush(&inner);
                }
                i += 1;
            }
            Elem::Verbatim { text, .. } => {
                lay.put(" ", text);
                i += 1;
            }
            Elem::Tok(t) if t.open.is_some_and(|c| broken.contains(&c)) => {
                let class = t.open.unwrap();
                if let Some(p) = prev {
                    lay.put(sep(p, t), t.text);
                } else {
                    lay.put("", t.text);
                }
                // the class nests relative to the line it opens on (which may be a `by` continuation)
                let open_indent: String =
                    lay.cur.chars().take_while(|c| c.is_whitespace()).collect();
                let items_indent = format!("{open_indent}{}", opts.indent);
                lay.flush(&items_indent);
                i += 1;
                let keeps_breaks = kept.contains(&class);
                i = fill_class(
                    elems,
                    i,
                    class,
                    keeps_breaks,
                    &mut lay,
                    &items_indent,
                    &open_indent,
                );
                prev = match &elems[i - 1] {
                    Elem::Tok(t) => Some(t),
                    _ => prev,
                };
            }
            Elem::Tok(t) if break_by && matches!(t.kind, Kind::ByKw | Kind::FromKw) => {
                lay.flush(&inner);
                lay.put("", t.text);
                prev = Some(t);
                i += 1;
            }
            Elem::Tok(t) => {
                let s = prev.map_or("", |p| sep(p, t));
                lay.put(s, t.text);
                prev = Some(t);
                i += 1;
            }
        }
    }
    if !lay.fresh {
        lay.lines.push(lay.cur.clone());
    }
    let _ = lay.opts;
    lay.lines
}

/// Fill the items of a broken class literal; returns the index just past its closing bracket.
///
/// With `keeps_breaks`, each line the author wrote starts a new line; any line
/// still too long wraps. Otherwise the members are packed to the width.
fn fill_class(
    elems: &[Elem],
    mut i: usize,
    class: u32,
    keeps_breaks: bool,
    lay: &mut Layout,
    inner: &str,
    base: &str,
) -> usize {
    let width = lay.opts.width;
    while i < elems.len() {
        match &elems[i] {
            Elem::Tok(t) if t.close == Some(class) => {
                lay.flush(base);
                lay.put("", t.text);
                return i + 1;
            }
            Elem::Comment {
                text, nl_before, ..
            } => {
                if *nl_before == 0 {
                    lay.trailing_comment(text, inner);
                } else {
                    if *nl_before >= 2 && !lay.lines.is_empty() {
                        lay.flush(inner);
                        lay.lines.push(String::new());
                    }
                    lay.flush(inner);
                    lay.put("", text);
                    lay.flush(inner);
                }
                i += 1;
            }
            Elem::Tok(first) => {
                // collect one unit (a glyph, range, named class, or predicate)
                let unit = first.class.map(|(_, u)| u);
                let mut toks = vec![first];
                let mut j = i + 1;
                while let Some(Elem::Tok(t)) = elems.get(j) {
                    if t.close == Some(class) || t.class.map(|(_, u)| u) != unit {
                        break;
                    }
                    toks.push(t);
                    j += 1;
                }
                let text = render_flat(&toks);
                if first.nl_before >= 2 && !lay.fresh {
                    lay.flush(inner);
                    lay.lines.push(String::new());
                }
                if keeps_breaks && first.nl_before >= 1 && !lay.fresh {
                    lay.flush(inner);
                }
                if !lay.fresh && lay.width() + 1 + text.chars().count() > width {
                    lay.flush(inner);
                }
                lay.put(" ", &text);
                i = j;
            }
            Elem::Verbatim { text, .. } => {
                lay.put(" ", text);
                i += 1;
            }
        }
    }
    i
}

/// Where a long contextual rule may start a new line.
#[derive(Default)]
struct Items {
    /// Element index of each glyph or class in the glyph sequence after the
    /// first, up to `by` or `from`, with its width written on one line.
    at: HashMap<usize, usize>,
    /// Fill lines to the width (a run of plain glyphs) rather than one item a
    /// line (a rule with bracketed classes).
    pack: bool,
    /// Width of the rule's first line before the first item, written on one line.
    head: usize,
}

/// The [`Items`] of a contextual rule; none for other statements, and for
/// `ignore` rules with several sequences.
fn sequence_items(elems: &[Elem]) -> Items {
    let toks: Vec<(usize, &Tok)> = elems
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            Elem::Tok(t) => Some((i, t)),
            _ => None,
        })
        .collect();
    let contextual = toks.iter().any(|(_, t)| {
        matches!(
            t.parent,
            Kind::BacktrackSequence | Kind::LookaheadSequence | Kind::ContextGlyphNode
        )
    });
    if !contextual || toks.iter().any(|(_, t)| t.kind == Kind::Comma) {
        return Items::default();
    }
    let end = toks
        .iter()
        .position(|(_, t)| matches!(t.kind, Kind::ByKw | Kind::FromKw))
        .unwrap_or(toks.len());
    let mut starts = Vec::new();
    for k in 1..end {
        let (_, t) = toks[k];
        let (_, prev) = toks[k - 1];
        let escaped = prev.kind == Kind::Backslash;
        let starts_item = t.class.is_none()
            && !escaped
            && (t.open.is_some()
                || matches!(
                    t.kind,
                    Kind::GlyphName | Kind::NamedGlyphClass | Kind::Cid | Kind::Backslash
                ));
        if starts_item && !matches!(prev.kind, Kind::SubKw | Kind::PosKw | Kind::RsubKw) {
            starts.push(k);
        }
    }
    let at = starts
        .iter()
        .enumerate()
        .map(|(n, &k)| {
            let next = starts.get(n + 1).copied().unwrap_or(end);
            let item: Vec<&Tok> = toks[k..next].iter().map(|(_, t)| *t).collect();
            (toks[k].0, render_flat(&item).chars().count())
        })
        .collect();
    let pack = !toks[..end].iter().any(|(_, t)| t.open.is_some());
    let first = starts.first().copied().unwrap_or(end);
    let head_toks: Vec<&Tok> = toks[..first].iter().map(|(_, t)| *t).collect();
    let head = render_flat(&head_toks).chars().count();
    Items { at, pack, head }
}

/// What layout a class literal in one statement needs.
struct ClassInfo {
    id: u32,
    /// Length of the class written on one line.
    flat_len: usize,
    /// The class must be a block: it holds a comment, a blank line, or keeps its breaks.
    must_break: bool,
    /// The author started the class on a new line after `[`, so their line
    /// breaks are kept (Prettier's rule for objects). Written on one line, it's packed.
    keeps_breaks: bool,
}

fn classes_in(elems: &[Elem]) -> Vec<ClassInfo> {
    let mut out: Vec<ClassInfo> = Vec::new();
    for e in elems {
        match e {
            Elem::Tok(t) if t.open.is_some() => out.push(ClassInfo {
                id: t.open.unwrap(),
                flat_len: 0,
                must_break: false,
                keeps_breaks: false,
            }),
            Elem::Tok(t) => {
                if let Some((c, unit)) = t.class
                    && let Some(entry) = out.iter_mut().find(|x| x.id == c)
                {
                    let first_member = entry.flat_len == 0;
                    entry.flat_len += t.text.len() + 1;
                    if first_member && t.nl_before >= 1 {
                        entry.keeps_breaks = true;
                        entry.must_break = true;
                    }
                    // a blank line between members is a group break the author
                    // made; one right after `[` isn't printed, so it can't
                    // force a block (or the next pass would undo it)
                    if t.nl_before >= 2 && unit > 0 {
                        entry.must_break = true;
                    }
                }
            }
            Elem::Comment {
                class: Some((c, _)),
                ..
            } => {
                if let Some(entry) = out.iter_mut().find(|x| x.id == *c) {
                    entry.must_break = true;
                }
            }
            _ => {}
        }
    }
    out
}

fn render_code(
    elems: &[Elem],
    trailing: Option<&str>,
    depth: usize,
    opts: &Settings,
) -> Vec<String> {
    let classes = classes_in(elems);
    let mut broken: HashSet<u32> = classes
        .iter()
        .filter(|c| c.must_break)
        .map(|c| c.id)
        .collect();
    let kept: HashSet<u32> = classes
        .iter()
        .filter(|c| c.keeps_breaks)
        .map(|c| c.id)
        .collect();
    let is_class_def = elems.iter().any(
        |e| matches!(e, Elem::Tok(t) if t.kind == Kind::Eq && t.parent == Kind::GlyphClassDefNode),
    );
    // in rules, only classes this long are worth turning into a block; shorter
    // ones read better on an overlong line than split over three
    let min_break = if is_class_def { 0 } else { opts.width / 3 };
    let overflows = |lines: &[String]| lines.iter().any(|l| l.chars().count() > opts.width);

    let mut break_by = false;
    let mut items = Items::default();
    let mut lines = layout_stmt(elems, depth, &broken, &kept, &items, break_by, opts);
    if overflows(&lines) && !is_class_def {
        // a contextual rule whose glyph sequence can't fit on one line reads best
        // one glyph or class per line, with `by` after the last when it fits.
        // Decided from the sequence alone, written unsplit, so every pass decides
        // the same; and only when each item fits on its own line, since otherwise
        // splitting the long class alone is more compact
        let candidate = sequence_items(elems);
        let base = opts.indent.chars().count() * depth;
        let inner = base + opts.indent.chars().count();
        let sequence = base + candidate.head + candidate.at.values().map(|l| l + 1).sum::<usize>();
        let each_fits = base + candidate.head <= opts.width
            && candidate.at.values().all(|&len| inner + len <= opts.width);
        if !candidate.at.is_empty() && sequence > opts.width && each_fits {
            items = candidate;
            lines = layout_stmt(elems, depth, &broken, &kept, &items, break_by, opts);
        }
        if overflows(&lines) {
            break_by = true;
            lines = layout_stmt(elems, depth, &broken, &kept, &items, break_by, opts);
        }
    }
    while overflows(&lines) {
        let candidate = classes
            .iter()
            .filter(|c| !broken.contains(&c.id) && c.flat_len >= min_break)
            .max_by_key(|c| c.flat_len);
        let Some(c) = candidate else { break };
        broken.insert(c.id);
        lines = layout_stmt(elems, depth, &broken, &kept, &items, break_by, opts);
    }
    // once a class is a block, `by` may fit back on the rule's line; prefer
    // that, since the next pass starts with the block (its rows are kept)
    // and would lay the rule out this way
    if break_by && !overflows(&lines) {
        let joined = layout_stmt(elems, depth, &broken, &kept, &items, false, opts);
        if !overflows(&joined) {
            lines = joined;
        }
    }

    if let Some(comment) = trailing {
        match lines.last_mut() {
            Some(last) => {
                last.push(' ');
                last.push_str(comment);
            }
            None => lines.push(comment.to_string()),
        }
    }
    lines
}

/// Format `source` as a whole file.
pub(crate) fn format_file(source: &str, opts: &Settings) -> Formatted {
    let tree = parse(source, Some(&parse_glyph_map(source)));
    let root = tree.root();
    let mut flat = Flattener {
        src: source,
        pos: 0,
        elems: Vec::new(),
        ws: false,
        nl: 0,
        ws_len: 0,
        next_class: 0,
        skipped: Vec::new(),
        off: None,
    };
    flat.walk_root(root);
    flat.close_off();
    let skipped = std::mem::take(&mut flat.skipped);
    // SAFETY-free lifetime juggling: elems borrow from `tree`, so finish inside this scope
    let stmts = keep_one_line_blocks(group(flat.elems), opts);
    let blanks = blank_lines(&stmts);
    let aligned = if opts.keep_alignment {
        align::aligned_runs(&stmts, opts)
    } else {
        Default::default()
    };

    let mut out = String::with_capacity(source.len());
    for (i, (placed, blank)) in stmts.iter().zip(blanks).enumerate() {
        if blank {
            out.push('\n');
        }
        match &placed.stmt {
            Stmt::Comment(text) => {
                out.push_str(&opts.indent.repeat(placed.depth));
                out.push_str(text);
                out.push('\n');
            }
            Stmt::Verbatim(text) => {
                out.push_str(&opts.indent.repeat(placed.depth));
                out.push_str(text.trim_end());
                out.push('\n');
            }
            Stmt::Code { .. } if aligned.contains_key(&i) => {
                out.push_str(aligned[&i].trim_end());
                out.push('\n');
            }
            Stmt::Code { elems, trailing } => {
                for line in render_code(elems, *trailing, placed.depth, opts) {
                    out.push_str(line.trim_end());
                    out.push('\n');
                }
            }
        }
    }
    // a broken region running to the end of the file (an unterminated string
    // while typing) must not gain a newline inside it
    let ends_broken = matches!(stmts.last().map(|p| &p.stmt), Some(Stmt::Verbatim(_)));
    if ends_broken && !source.ends_with('\n') {
        out.pop();
    }
    Formatted {
        text: out,
        untouched: skipped,
        feature_body: false,
    }
}

/// Format a feature *body* (Glyphs/Inter style, no `feature xxxx { }` wrapper).
/// Format `source` as a feature body, by wrapping it in a `feature` block.
/// `None` when the wrapper can't be removed cleanly afterwards: an untouched
/// region that runs into it (the body has an unclosed block, say).
pub(crate) fn format_body(source: &str, opts: &Settings) -> Option<Formatted> {
    const OPEN: &str = "feature zzzz {\n";
    const CLOSE: &str = "\n} zzzz;\n";
    let wrapped = format!("{OPEN}{source}{CLOSE}");
    let Formatted {
        text,
        untouched: skipped,
        ..
    } = format_file(&wrapped, opts);
    let inside = OPEN.len()..OPEN.len() + source.len();
    if skipped
        .iter()
        .any(|r| r.start < inside.start || r.end > inside.end)
    {
        return None;
    }
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.first() != Some(&OPEN.trim_end()) || lines.last() != Some(&CLOSE.trim()) {
        return None;
    }
    lines.remove(0);
    lines.pop();
    let mut body = String::with_capacity(text.len());
    for line in lines {
        body.push_str(line.strip_prefix(opts.indent.as_str()).unwrap_or(line));
        body.push('\n');
    }
    let shift = OPEN.len();
    let skipped = skipped
        .into_iter()
        .map(|r| r.start.saturating_sub(shift)..r.end.saturating_sub(shift))
        .collect();
    Some(Formatted {
        text: body,
        untouched: skipped,
        feature_body: true,
    })
}

/// Parse a single in-memory file (includes are not followed).
pub fn parse(source: &str, glyphs: Option<&GlyphMap>) -> ParseTree {
    parse_with_diagnostics(source, glyphs).0
}

/// Where the word `include` appears without a `(` after it, outside comments and
/// strings. fea-rs panics when it resolves an include that has no path, which
/// happens whenever someone is partway through typing one, so the parser never
/// sees these.
pub(crate) fn bare_includes(source: &str) -> Vec<Range<usize>> {
    let is_name = |c: char| c.is_ascii_alphanumeric() || "_.-\\*+:^|~@'$".contains(c);
    let mut out = Vec::new();
    let mut chars = source.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        match c {
            '#' => while chars.next_if(|&(_, c)| c != '\n' && c != '\r').is_some() {},
            '"' => {
                while chars.next_if(|&(_, c)| c != '"').is_some() {}
                chars.next();
            }
            c if is_name(c) => {
                let mut end = at + c.len_utf8();
                while let Some((i, c)) = chars.next_if(|&(_, c)| is_name(c)) {
                    end = i + c.len_utf8();
                }
                let followed_by_paren = source[end..].trim_start().starts_with('(');
                if &source[at..end] == "include" && !followed_by_paren {
                    out.push(at..end);
                }
            }
            _ => {}
        }
    }
    out
}

/// `source` with each bare `include` replaced by `INCLUDE`, which fea-rs reads
/// as a name; the length is the same, so every byte offset still holds.
fn hide_bare_includes(source: &str) -> std::borrow::Cow<'_, str> {
    let bare = bare_includes(source);
    if bare.is_empty() {
        return std::borrow::Cow::Borrowed(source);
    }
    let mut text = source.to_string();
    for range in bare {
        text.replace_range(range, "INCLUDE");
    }
    std::borrow::Cow::Owned(text)
}

/// Parse a single in-memory file, keeping fea-rs's parse diagnostics.
pub(crate) fn parse_with_diagnostics(
    source: &str,
    glyphs: Option<&GlyphMap>,
) -> (ParseTree, fea_rs::DiagnosticSet) {
    const NAME: &str = "<memory>";
    let text: Arc<str> = hide_bare_includes(source).into();
    let resolver = move |p: &std::path::Path| {
        if p == std::path::Path::new(NAME) {
            Ok(text.clone())
        } else {
            Err(fea_rs::parse::SourceLoadError::new(
                p.to_path_buf(),
                "includes are not followed",
            ))
        }
    };
    fea_rs::parse::parse_root(NAME.into(), glyphs, Box::new(resolver))
        .expect("root source resolves")
}

/// Keep a block on one line when the author wrote it on one line, it has no
/// comments or nested blocks, and it still fits the width (Prettier's rule
/// for object literals). Everything else is expanded one statement per line.
fn keep_one_line_blocks<'a>(stmts: Vec<Placed<'a>>, opts: &Settings) -> Vec<Placed<'a>> {
    let mut out = Vec::with_capacity(stmts.len());
    let mut i = 0;
    while i < stmts.len() {
        match one_line_block(&stmts, i, opts) {
            Some((merged, end)) => {
                out.push(merged);
                i = end + 1;
            }
            None => {
                out.push(stmts[i].clone());
                i += 1;
            }
        }
    }
    out
}

fn one_line_block<'a>(
    stmts: &[Placed<'a>],
    i: usize,
    opts: &Settings,
) -> Option<(Placed<'a>, usize)> {
    let open = &stmts[i];
    let Stmt::Code {
        elems,
        trailing: None,
    } = &open.stmt
    else {
        return None;
    };
    if !open.stmt.opens_block() {
        return None;
    }
    let mut merged = elems.clone();
    let mut k = i + 1;
    loop {
        let next = stmts.get(k)?;
        if next.nl_before > 0 {
            return None;
        }
        let Stmt::Code { elems, trailing } = &next.stmt else {
            return None;
        };
        if next.stmt.opens_block() {
            return None;
        }
        merged.extend(elems.iter().cloned());
        if next.stmt.closes_block() {
            let has_comment = merged.iter().any(|e| matches!(e, Elem::Comment { .. }));
            // a class whose line breaks are kept can't sit on one line
            let keeps_breaks = classes_in(&merged).iter().any(|c| c.keeps_breaks);
            if has_comment || keeps_breaks {
                return None;
            }
            let lines = layout_stmt(
                &merged,
                open.depth,
                &HashSet::new(),
                &HashSet::new(),
                &Items::default(),
                false,
                opts,
            );
            let fits = lines.len() == 1 && lines[0].chars().count() <= opts.width;
            if !fits {
                return None;
            }
            let stmt = Stmt::Code {
                elems: merged,
                trailing: *trailing,
            };
            return Some((
                Placed {
                    stmt,
                    depth: open.depth,
                    nl_before: open.nl_before,
                },
                k,
            ));
        }
        if trailing.is_some() {
            return None;
        }
        k += 1;
    }
}

/// Every glyph-like name in the file, for building a parse-only glyph order.
///
/// A byte scan rather than a parse (which would double the cost). It doesn't
/// skip comments or strings: a superset is harmless, and it makes each token's
/// reading depend only on its own text. Every hyphenated token is then in the
/// map, so fea-rs reads it as a glyph name (never splits it as a range), whether
/// the text is a whole document or one item of it.
pub(crate) fn file_glyph_names(
    source: &str,
    hyphenated_only: bool,
) -> std::collections::BTreeSet<String> {
    let bytes = source.as_bytes();
    let is_name = |b: u8| {
        b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'.' | b'_' | b'-' | b'+' | b'*' | b':' | b'^' | b'|' | b'~'
            )
    };
    let mut names = std::collections::BTreeSet::new();
    let mut i = 0;
    while i < bytes.len() {
        if !is_name(bytes[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && is_name(bytes[i]) {
            i += 1;
        }
        let word = &source[start..i];
        if !hyphenated_only || word.contains('-') {
            names.insert(word.to_string());
        }
    }
    names
}

/// The glyph order the formatter parses with: the font's glyphs (if known)
/// plus every name in the file.
///
/// fea-rs needs a glyph order to tell `lam-ar.init` (a glyph) from `a-z` (a
/// range); without it, contextual rules that use hyphenated names fail to
/// parse, and with only the font's glyphs, a stale name fails too. Treating
/// every name as a glyph can misread a range as one glyph, but formatting
/// never changes token text, so the output is the same.
pub fn parse_glyph_map(source: &str) -> GlyphMap {
    // only hyphenated names need disambiguating, and every one in the text is
    // collected; the font's own names add nothing to the parse (and copying
    // thousands of them per parse dominated incremental linting)
    glyph_map_with_file_names(source, None, true)
}

/// The font's glyphs plus names found in the file (all of them, or only hyphenated ones).
pub fn glyph_map_with_file_names(
    source: &str,
    font: Option<&GlyphMap>,
    hyphenated_only: bool,
) -> GlyphMap {
    let mut names = file_glyph_names(source, hyphenated_only);
    if let Some(font) = font {
        for ident in font.iter() {
            if let fea_rs::GlyphIdent::Name(n) = ident {
                names.insert(n.to_string());
            }
        }
    }
    let idents = names
        .into_iter()
        .map(|n| fea_rs::GlyphIdent::Name(n.as_str().into()));
    GlyphMap::new(idents)
        .unwrap_or_else(|_| GlyphMap::new(std::iter::empty::<fea_rs::GlyphIdent>()).unwrap())
}

/// The indent unit a file already uses: the most common step in leading
/// whitespace from a line ending in `{` or `[` to the line after it.
pub fn detect_indent(source: &str) -> Option<String> {
    let mut counts: std::collections::HashMap<String, usize> = Default::default();
    let mut prev: Option<&str> = None;
    for line in source.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(opener) = prev {
            let outer = opener.len() - opener.trim_start().len();
            let inner = line.len() - line.trim_start().len();
            if inner > outer && line.starts_with(&opener[..outer]) {
                *counts.entry(line[outer..inner].to_string()).or_default() += 1;
            }
        }
        let code = line.split('#').next().unwrap_or("").trim_end();
        prev = (code.ends_with('{') || code.ends_with('[')).then_some(line);
    }
    counts
        .into_iter()
        .max_by_key(|(unit, n)| (*n, std::cmp::Reverse(unit.len())))
        .map(|(unit, _)| unit)
}
