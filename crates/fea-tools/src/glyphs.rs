//! The font's glyph names, for rules that check names against the font.

use std::collections::HashSet;

/// A set of glyph names. Order doesn't matter: no rule here depends on glyph IDs.
///
/// ```
/// use fea_tools::GlyphNames;
///
/// let glyphs: GlyphNames = ["a", "b", "a.sc"].into_iter().collect();
/// assert!(glyphs.contains("a.sc"));
/// assert!(!glyphs.contains("c"));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GlyphNames {
    names: HashSet<String>,
}

impl GlyphNames {
    /// Whether the font has a glyph called `name`.
    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// How many glyphs there are.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether there are no glyphs.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// The names, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }
}

impl<S: Into<String>> FromIterator<S> for GlyphNames {
    fn from_iter<I: IntoIterator<Item = S>>(iter: I) -> Self {
        GlyphNames {
            names: iter.into_iter().map(Into::into).collect(),
        }
    }
}
