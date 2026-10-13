//! Incremental linting for editors: re-parse only the top-level items an edit touches.
//!
//! The document is split into chunks at top-level item boundaries (each chunk is
//! an item plus the trivia before it). Each chunk caches its facts; diagnostics
//! come from combining the cached facts, so an edit costs one small parse plus
//! the combine step.
//!
//! After an edit, the touched chunks and one neighbour on each side are parsed
//! again as a region. If that region ends in a broken item (an unclosed `{` that,
//! in the whole document, would swallow what follows), the region can't be
//! trusted in isolation and the whole document is rebuilt.
//!
//! The diagnostics are always the ones a full [`lint`](crate::lint()) of the
//! same text would report; the test suite checks this after random edits.

use std::ops::Range;

use fea_rs::{Kind, NodeOrToken};

use crate::diagnostic::Diagnostic;
use crate::fmt::{parse, parse_glyph_map};
use crate::glyphs::GlyphNames;
use crate::lint::{Facts, LintOptions, Suggestions, analyze, combine};

struct Chunk {
    start: usize,
    len: usize,
    facts: Facts,
}

/// A document that stays linted as it's edited.
///
/// ```
/// use feanor::{Incremental, LintOptions, Rule};
///
/// let mut doc = Incremental::new("@a = [x y];\n".into(), LintOptions::default());
/// doc.edit(9..9, " x"); // now "@a = [x y x];"
/// assert!(doc.diagnostics().iter().any(|d| d.rule == Rule::DuplicateGlyphInClass));
/// ```
pub struct Incremental {
    text: String,
    chunks: Vec<Chunk>,
    glyphs: Option<GlyphNames>,
    project: bool,
    suggestions: Suggestions,
}

/// What an [`Incremental::edit`] cost.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct EditStats {
    /// Bytes parsed again for this edit.
    pub reparsed_bytes: usize,
    /// The edit left an unclosed block, so the whole document was parsed again.
    pub full_rebuild: bool,
}

/// Split `text` at top-level item boundaries. A broken item never ends a chunk:
/// alone, its parse error would land at the chunk's end instead of on the next
/// token, as it does in the whole document. The flag is false when the text
/// ends in a broken item, so a region can't be trusted in isolation.
fn split(text: &str) -> (Vec<Range<usize>>, bool) {
    let tree = parse(text, Some(&parse_glyph_map(text)));
    let mut out = Vec::new();
    let mut start = 0;
    let mut pos = 0;
    let mut broken = false;
    for child in tree.root().iter_children() {
        pos += child.text_len();
        match child {
            NodeOrToken::Node(n) if has_error(n) => broken = true,
            NodeOrToken::Node(_) => {
                // a clean item ends the chunk (merging any broken items before it)
                broken = false;
                out.push(start..pos);
                start = pos;
            }
            NodeOrToken::Token(t)
                if !matches!(t.kind, Kind::Whitespace | Kind::Comment | Kind::Eof) =>
            {
                broken = true
            }
            NodeOrToken::Token(_) => {}
        }
    }
    let trustworthy = !broken;
    // trailing trivia (or a broken tail) joins the last chunk
    if start < text.len() {
        match out.last_mut() {
            Some(last) if trustworthy => last.end = text.len(),
            _ => out.push(start..text.len()),
        }
    }
    (out, trustworthy)
}

fn has_error(n: &fea_rs::Node) -> bool {
    n.error
        || n.iter_children()
            .any(|c| c.as_node().is_some_and(has_error))
}

impl Incremental {
    /// Lint `text` and keep what's needed to re-lint it after edits. The
    /// options, including a copy of the glyph names, last as long as the document.
    pub fn new(text: String, options: LintOptions) -> Self {
        let mut doc = Incremental {
            text,
            chunks: Vec::new(),
            glyphs: options.glyphs.cloned(),
            project: options.project,
            suggestions: Suggestions::new(),
        };
        doc.rebuild();
        doc
    }

    fn rebuild(&mut self) {
        let options = LintOptions {
            glyphs: self.glyphs.as_ref(),
            project: self.project,
        };
        let (ranges, _) = split(&self.text);
        self.chunks = ranges
            .into_iter()
            .map(|r| Chunk {
                start: r.start,
                len: r.len(),
                facts: analyze(&self.text[r], &options),
            })
            .collect();
    }

    /// The current text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replace bytes `range` of the text with `new_text`, then re-lint what it touched.
    ///
    /// # Panics
    ///
    /// Panics when `range` is out of bounds or doesn't fall on UTF-8 character
    /// boundaries, as [`String::replace_range`] does.
    pub fn edit(&mut self, range: Range<usize>, new_text: &str) -> EditStats {
        self.text.replace_range(range.clone(), new_text);
        let removed = range.len();
        let shift = |at: usize| at + new_text.len() - removed;
        if self.chunks.is_empty() {
            self.rebuild();
            return EditStats {
                reparsed_bytes: self.text.len(),
                full_rebuild: true,
            };
        }

        // chunks touching the edit (inclusive at both ends), plus a neighbour each side
        let first = self
            .chunks
            .partition_point(|c| c.start + c.len < range.start);
        let last = self
            .chunks
            .partition_point(|c| c.start <= range.end)
            .saturating_sub(1)
            .max(first);
        let lo = first.saturating_sub(1);
        let hi = (last + 1).min(self.chunks.len() - 1);
        let region_start = self.chunks[lo].start;
        let region_end = shift(self.chunks[hi].start + self.chunks[hi].len);
        let at_doc_end = hi == self.chunks.len() - 1;

        let (ranges, trustworthy) = split(&self.text[region_start..region_end]);
        if !trustworthy && !at_doc_end {
            self.rebuild();
            return EditStats {
                reparsed_bytes: self.text.len(),
                full_rebuild: true,
            };
        }
        let options = LintOptions {
            glyphs: self.glyphs.as_ref(),
            project: self.project,
        };
        let region = &self.text[region_start..region_end];
        let fresh: Vec<Chunk> = ranges
            .into_iter()
            .map(|r| Chunk {
                start: region_start + r.start,
                len: r.len(),
                facts: analyze(&region[r], &options),
            })
            .collect();
        for c in &mut self.chunks[hi + 1..] {
            c.start = shift(c.start);
        }
        self.chunks.splice(lo..=hi, fresh);
        EditStats {
            reparsed_bytes: region_end - region_start,
            full_rebuild: false,
        }
    }

    /// The named things in the current text, as [`symbols`](crate::symbols()) finds them.
    pub fn symbols(&self) -> crate::Symbols {
        crate::Symbols::from_pieces(self.chunks.iter().map(|c| (c.start, &c.facts)))
    }

    /// The diagnostics for the current text, as [`lint`](crate::lint()) reports them
    /// for a whole document (feature bodies aren't detected here).
    pub fn diagnostics(&mut self) -> Vec<Diagnostic> {
        let options = LintOptions {
            glyphs: self.glyphs.as_ref(),
            project: self.project,
        };
        combine(
            self.chunks.iter().map(|c| (c.start, &c.facts)),
            &options,
            &mut self.suggestions,
        )
    }
}
