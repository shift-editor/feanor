#![warn(missing_docs)]

//! A formatter and linter for OpenType feature files (`.fea`), built on
//! [fea-rs](https://docs.rs/fea-rs)'s lossless syntax tree.
//!
//! - [`format()`] changes layout only: spacing, indentation, line breaks and
//!   blank lines. Every token and comment comes out unchanged and in order.
//! - [`lint`] reports [`Diagnostic`]s for a set of [`Rule`]s, each with a
//!   stable code (`G001`), and offers [`Fix`]es marked safe or unsafe.
//! - [`Project`] lints many files as one document: FEA `include()`,
//!   C-preprocessor `#include` / `#ifdef`, and Glyphs-style sections.
//! - [`Incremental`] keeps a document linted as it's edited.
//! - [`Config`] reads `feanor.toml`.
//!
//! Every range is a byte range into the text given; [`LineIndex`] converts to
//! the line and UTF-16 column that editors and the Language Server Protocol use.
//!
//! ```
//! use feanor::{format, lint, FormatOptions, GlyphNames, LintOptions, Rule};
//!
//! let source = "feature smcp {\n    sub a by a.sc;\n    sub b by bsc;\n} smcp;\n";
//! assert_eq!(format(source, &FormatOptions::default()).text, source);
//!
//! let glyphs: GlyphNames = ["a", "b", "a.sc", "b.sc"].into_iter().collect();
//! let diagnostics = lint(source, &LintOptions::default().with_glyphs(&glyphs));
//! assert_eq!(diagnostics.len(), 1);
//! assert_eq!(diagnostics[0].rule, Rule::GlyphNotInFont);
//! assert_eq!(diagnostics[0].message, "glyph 'bsc' is not in the font; did you mean 'b.sc'?");
//! ```

pub mod config;
mod diagnostic;
mod fmt;
mod glyphs;
mod incremental;
mod lint;
pub mod project;

pub use config::{Config, ConfigError, ParseError, SettingError};
pub use diagnostic::{
    Applicability, Diagnostic, Fix, LineIndex, Position, Related, Rule, Selection, Severity,
    TextEdit, apply_fixes, apply_noqa,
};
pub use fmt::{FormatOptions, Formatted, Indent, detect_indent, format};
pub use glyphs::GlyphNames;
pub use incremental::{EditStats, Incremental};
pub use lint::{LintOptions, lint};
pub use project::{
    BuildConfig, FileDiagnostic, Problem, ProblemKind, Project, ProjectError, configurations,
};

/// Helpers for this crate's own tools and tests. Not part of the stable API.
#[doc(hidden)]
pub mod internal {
    pub use crate::fmt::{glyph_map_with_file_names, parse, parse_glyph_map};

    /// Lint `source` as one whole document: no feature-body detection, no
    /// project. The reference [`Incremental`](crate::Incremental) must match.
    pub fn lint_whole_document(
        source: &str,
        options: &crate::LintOptions,
    ) -> Vec<crate::Diagnostic> {
        crate::lint::lint_document(source, options, &mut Default::default())
    }

    /// Every token except whitespace and parser placeholders, comments
    /// included (with trailing whitespace trimmed, which the formatter removes).
    pub fn significant_tokens(source: &str) -> Vec<(fea_rs::Kind, String)> {
        let tree = parse(source, None);
        tree.root()
            .iter_tokens()
            .filter(|t| !matches!(t.kind, fea_rs::Kind::Whitespace | fea_rs::Kind::Eof))
            .map(|t| {
                let text = if t.kind == fea_rs::Kind::Comment {
                    t.text.trim_end()
                } else {
                    t.text.as_str()
                };
                (t.kind, text.to_string())
            })
            .collect()
    }

    /// The first difference between two texts' significant tokens, if any.
    pub fn first_token_mismatch(a: &str, b: &str) -> Option<String> {
        let (ta, tb) = (significant_tokens(a), significant_tokens(b));
        match ta.iter().zip(&tb).position(|(x, y)| x != y) {
            Some(i) => Some(format!("token {i}: {:?} vs {:?}", ta[i], tb[i])),
            None if ta.len() != tb.len() => Some(format!("length {} vs {}", ta.len(), tb.len())),
            None => None,
        }
    }
}

// the README's example is compiled and run as a doctest, so it can't go stale
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
