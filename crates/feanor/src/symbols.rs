//! Where named things are defined and used: glyph classes, mark classes,
//! lookups, and glyph names. Editors build go-to-definition, find-references,
//! and rename on this; the linter's unused and undefined rules read the same facts.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::diagnostic::TextEdit;
use crate::lint::{Facts, LintOptions, analyze};

/// What kind of name a [`Symbol`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum SymbolKind {
    /// A glyph class: `@name = [...]`.
    Class,
    /// A mark class, defined by one or more `markClass` statements.
    MarkClass,
    /// A named lookup: `lookup Name { ... } Name;`.
    Lookup,
    /// A glyph name. Glyphs are defined by the font, so these have references only.
    Glyph,
}

/// One named thing and every place it's written.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Symbol {
    /// What it names.
    pub kind: SymbolKind,
    /// The name as written: classes and mark classes with their `@`.
    pub name: String,
    /// Byte ranges of the name where it's defined, in text order: one for a
    /// class or lookup, one per statement for a mark class, none for a glyph
    /// or for a name that's used but never defined.
    pub definitions: Vec<Range<usize>>,
    /// Byte ranges of the name everywhere else it's written, in text order.
    pub references: Vec<Range<usize>>,
}

impl Symbol {
    /// Every range the name is written at, definitions first.
    pub fn occurrences(&self) -> impl Iterator<Item = &Range<usize>> {
        self.definitions.iter().chain(&self.references)
    }
}

/// The named things in one text. See [`symbols()`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Symbols {
    symbols: Vec<Symbol>,
}

impl Symbols {
    /// Every symbol, ordered by kind and then name.
    pub fn iter(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.iter()
    }

    /// The symbol of `kind` called `name` (with its `@` for classes).
    pub fn get(&self, kind: SymbolKind, name: &str) -> Option<&Symbol> {
        self.symbols
            .iter()
            .find(|symbol| symbol.kind == kind && symbol.name == name)
    }

    /// The symbol written at byte `offset`, counting an offset just past a
    /// name as on it, as a cursor right after a word is.
    pub fn at(&self, offset: usize) -> Option<&Symbol> {
        self.symbols.iter().find(|symbol| {
            symbol
                .occurrences()
                .any(|range| range.start <= offset && offset <= range.end)
        })
    }

    /// The edits that rename the class, mark class, or lookup at byte
    /// `offset` to `new_name`, everywhere it's written. A class's `@` is kept
    /// whether or not `new_name` starts with one. `None` when no renameable
    /// symbol is at `offset` or `new_name` isn't a valid name; glyphs are
    /// renamed in the font, not here.
    pub fn rename(&self, offset: usize, new_name: &str) -> Option<Vec<TextEdit>> {
        let symbol = self.at(offset)?;
        let bare = new_name.strip_prefix('@').unwrap_or(new_name);
        if !is_valid_name(bare) {
            return None;
        }
        let new_text = match symbol.kind {
            SymbolKind::Class | SymbolKind::MarkClass => format!("@{bare}"),
            SymbolKind::Lookup => bare.to_string(),
            SymbolKind::Glyph => return None,
        };
        Some(
            symbol
                .occurrences()
                .map(|range| TextEdit {
                    range: range.clone(),
                    new_text: new_text.clone(),
                })
                .collect(),
        )
    }

    pub(crate) fn from_pieces<'f>(pieces: impl Iterator<Item = (usize, &'f Facts)>) -> Self {
        let mut table: BTreeMap<(SymbolKind, String), Symbol> = BTreeMap::new();
        let mut mark_classes: BTreeMap<String, ()> = BTreeMap::new();
        let mut class_uses = Vec::new();
        let at = |offset: usize, range: &Range<usize>| range.start + offset..range.end + offset;
        for (offset, facts) in pieces {
            for (name, range) in facts.class_definitions() {
                entry(&mut table, SymbolKind::Class, name)
                    .definitions
                    .push(at(offset, range));
            }
            for (name, range) in facts.mark_class_definitions() {
                mark_classes.insert(name.to_string(), ());
                entry(&mut table, SymbolKind::MarkClass, name)
                    .definitions
                    .push(at(offset, range));
            }
            for (name, range) in facts.lookup_definitions() {
                entry(&mut table, SymbolKind::Lookup, name)
                    .definitions
                    .push(at(offset, range));
            }
            for (name, range) in facts.lookup_references() {
                entry(&mut table, SymbolKind::Lookup, name)
                    .references
                    .push(at(offset, range));
            }
            for (name, range) in facts.glyph_references() {
                entry(&mut table, SymbolKind::Glyph, name)
                    .references
                    .push(at(offset, range));
            }
            class_uses.extend(
                facts
                    .class_references()
                    .map(|(name, range)| (name.to_string(), at(offset, range))),
            );
        }
        // A use names a mark class when any `markClass` statement defines it.
        for (name, range) in class_uses {
            let kind = if mark_classes.contains_key(&name) {
                SymbolKind::MarkClass
            } else {
                SymbolKind::Class
            };
            entry(&mut table, kind, &name).references.push(range);
        }
        let mut symbols: Vec<Symbol> = table.into_values().collect();
        for symbol in &mut symbols {
            symbol.definitions.sort_by_key(|range| range.start);
            symbol.references.sort_by_key(|range| range.start);
        }
        Symbols { symbols }
    }
}

fn entry<'t>(
    table: &'t mut BTreeMap<(SymbolKind, String), Symbol>,
    kind: SymbolKind,
    name: &str,
) -> &'t mut Symbol {
    table
        .entry((kind, name.to_string()))
        .or_insert_with(|| Symbol {
            kind,
            name: name.to_string(),
            definitions: Vec::new(),
            references: Vec::new(),
        })
}

/// The named things in `source`: where each glyph class, mark class, and
/// lookup is defined and used, and where each glyph name is written. Ranges
/// are byte ranges into `source`; [`crate::LineIndex`] converts them for editors.
///
/// ```
/// use feanor::{symbols, SymbolKind};
///
/// let source = "@lc = [a b];\nfeature smcp { sub @lc by @lc.sc; } smcp;\n";
/// let symbols = symbols(source);
/// let lc = symbols.get(SymbolKind::Class, "@lc").unwrap();
/// assert_eq!(&source[lc.definitions[0].clone()], "@lc");
/// assert_eq!(lc.references.len(), 1);
/// assert_eq!(symbols.at(source.find("@lc by").unwrap()), Some(lc));
/// ```
pub fn symbols(source: &str) -> Symbols {
    let facts = analyze(source, &LintOptions::default());
    Symbols::from_pieces(std::iter::once((0, &facts)))
}

/// Whether `name` can name a class or lookup: a letter or `_` first, then
/// letters, digits, `_`, `.`, or `-`, as the FEA specification allows.
fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        && name.len() <= 63
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply_fixes;

    const SOURCE: &str = "\
@lc = [a b];
@uc = [A B];
markClass [acute] <anchor 0 500> @TOP;
markClass [grave] <anchor 0 500> @TOP;
lookup SMALL { sub @lc by @lc.sc; } SMALL;
feature smcp { lookup SMALL; } smcp;
feature c2sc { sub @uc by @lc.sc; } c2sc;
feature mark { pos base @lc <anchor 250 450> mark @TOP; } mark;
";

    fn text<'s>(source: &'s str, range: &Range<usize>) -> &'s str {
        &source[range.clone()]
    }

    #[test]
    fn classes_are_found_at_their_definition_and_every_use() {
        let lc = symbols(SOURCE);
        let lc = lc.get(SymbolKind::Class, "@lc").unwrap();

        assert_eq!(lc.definitions.len(), 1);
        assert_eq!(lc.references.len(), 2);
        assert!(lc.occurrences().all(|range| text(SOURCE, range) == "@lc"));
    }

    #[test]
    fn a_mark_class_collects_every_markclass_statement_and_its_uses() {
        let symbols = symbols(SOURCE);
        let top = symbols.get(SymbolKind::MarkClass, "@TOP").unwrap();

        assert_eq!(top.definitions.len(), 2);
        assert_eq!(top.references.len(), 1);
        assert_eq!(symbols.get(SymbolKind::Class, "@TOP"), None);
    }

    #[test]
    fn lookups_link_the_block_to_its_references() {
        let symbols = symbols(SOURCE);
        let reference = SOURCE.find("lookup SMALL;").unwrap() + "lookup ".len();

        let small = symbols.at(reference).unwrap();

        assert_eq!(small.kind, SymbolKind::Lookup);
        assert_eq!(text(SOURCE, &small.definitions[0]), "SMALL");
        assert_eq!(small.definitions[0].start, SOURCE.find("SMALL").unwrap());
    }

    #[test]
    fn glyph_names_are_references_without_their_escape() {
        let source = "feature liga { sub \\f i by f_i; } liga;\n";
        let symbols = symbols(source);

        let f = symbols.get(SymbolKind::Glyph, "f").unwrap();

        assert_eq!(text(source, &f.references[0]), "f");
        assert!(f.definitions.is_empty());
        assert!(symbols.get(SymbolKind::Glyph, "f_i").is_some());
    }

    #[test]
    fn renaming_a_class_rewrites_every_occurrence_and_keeps_the_at_sign() {
        let symbols = symbols(SOURCE);
        let edits = symbols
            .rename(SOURCE.find("@lc").unwrap() + 1, "lower")
            .unwrap();
        let fix = crate::Fix::new("Rename", edits, crate::Applicability::Safe);
        let diagnostic = crate::Diagnostic {
            rule: crate::Rule::Syntax,
            severity: crate::Severity::Hint,
            range: 0..0,
            message: String::new(),
            fixes: vec![fix],
            related: None,
        };

        let renamed = apply_fixes(SOURCE, &[diagnostic], crate::Applicability::Safe);

        assert_eq!(
            renamed.matches("@lower ").count() + renamed.matches("@lower;").count(),
            3
        );
        assert!(!renamed.contains("@lc "));
        assert!(
            renamed.contains("@lc.sc"),
            "a different class sharing a prefix is untouched"
        );
    }

    #[test]
    fn an_edited_document_finds_the_same_symbols_as_a_fresh_scan() {
        let mut document = crate::Incremental::new(SOURCE.to_string(), LintOptions::default());
        let at = SOURCE.find("feature c2sc").unwrap();

        document.edit(at..at, "@extra = [@lc a];\n");

        assert_eq!(document.symbols(), symbols(document.text()));
        let lc = document.symbols();
        assert_eq!(lc.get(SymbolKind::Class, "@lc").unwrap().references.len(), 3);
    }

    #[test]
    fn names_that_are_invalid_or_glyphs_are_not_renamed() {
        let symbols = symbols(SOURCE);
        let class = SOURCE.find("@lc").unwrap();
        let glyph = SOURCE.find("acute").unwrap();

        assert_eq!(symbols.rename(class, "1lower"), None);
        assert_eq!(symbols.rename(class, "low er"), None);
        assert_eq!(symbols.rename(glyph, "acutecomb"), None);
    }
}
