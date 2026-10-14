//! Check that two versions of a feature file make a font behave the same.
//!
//! Each version is compiled with fea-rs into a stand-in font: every glyph is
//! an empty box with the same advance and its own private-use code point, so
//! any glyph sequence can be shaped. The sequences come from the glyphs the
//! change touches: the glyph names written in the part of the file that
//! differs, and the members of classes named there. Every single glyph and
//! pair of them is shaped (a sample when there are too many), plus random
//! longer runs, with every feature on and then with each feature alone, under
//! every script and language the file registers. Output glyphs and positions
//! must match.
//!
//! This answers "does the change alter what the font does", not "would a
//! reader notice": that depends on the real outlines. And it tests a sample,
//! so [`Outcome::SameBehaviour`] means no difference was found, not that none
//! exists. Identical compiled tables ([`Outcome::SameTables`]) are proof.
//!
//! It compiles with fea-rs, the compiler in fontc. Where fontTools reads a
//! file differently (it rejects some things fea-rs accepts), the result
//! speaks for fea-rs only.

use std::collections::BTreeSet;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use fea_rs::compile::{Compilation, NopFeatureProvider, NopVariationInfo, Opts};
use fea_rs::{GlyphIdent, GlyphMap};
use harfrust::{FontRef, ShaperData, Tag, UnicodeBuffer};
use write_fonts::FontBuilder;
use write_fonts::tables::cmap::Cmap;
use write_fonts::tables::head::Head;
use write_fonts::tables::hhea::Hhea;
use write_fonts::tables::hmtx::{Hmtx, LongMetric};
use write_fonts::tables::maxp::Maxp;
use write_fonts::types::GlyphId;

use crate::GlyphNames;
use crate::symbols::{SymbolKind, symbols};

/// The first code point given to a glyph in the stand-in font.
const FIRST_CODE_POINT: u32 = 0xF0000;
/// The last usable code point in that plane, which caps the glyph count.
const LAST_CODE_POINT: u32 = 0xFFFFD;
const ADVANCE: u16 = 500;
/// How many pairs, and how many longer runs, to shape at most per script.
const SAMPLE: usize = 4000;

/// What [`verify`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// The two versions compile to the same tables.
    SameTables,
    /// The tables differ, but every sequence tried shaped the same.
    SameBehaviour {
        /// How many times a sequence was shaped with each version, counting
        /// each script, language and feature setting.
        sequences: usize,
    },
    /// A sequence shaped differently.
    Differs(Difference),
    /// The first version compiles and the second doesn't.
    Breaks {
        /// fea-rs's errors for the second version.
        errors: String,
    },
}

/// A glyph sequence that shapes differently in the two versions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Difference {
    /// The glyph names shaped.
    pub input: Vec<String>,
    /// The OpenType script and language tags it was shaped under.
    pub script: String,
    /// The language tag, if not the default.
    pub language: Option<String>,
    /// The one feature that was on, or `None` when all of them were.
    pub feature: Option<String>,
    /// The output of the first version: glyph names, with any positioning.
    pub before: Vec<String>,
    /// The output of the second version.
    pub after: Vec<String>,
}

/// Why [`verify`] couldn't compare.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VerifyError {
    /// The first version doesn't compile, so there's nothing to compare with.
    #[error("the original doesn't compile:\n{0}")]
    SourceDoesNotCompile(String),
    /// More glyphs than the stand-in font can map.
    #[error("too many glyphs to verify ({0})")]
    TooManyGlyphs(usize),
}

/// Options for [`verify`].
#[derive(Clone, Copy, Debug, Default)]
pub struct VerifyOptions<'a> {
    /// The font's glyph names. Without them, the glyph names written in the
    /// files are used, so ranges like `a.sc - d.sc` won't compile.
    pub glyphs: Option<&'a GlyphNames>,
    /// The file's path, so `include` statements resolve relative to it.
    pub path: Option<&'a Path>,
}

/// Compare what `before` and `after` make a font do. See the module
/// documentation for how, and what a result does and doesn't prove.
///
/// # Errors
///
/// [`VerifyError::SourceDoesNotCompile`] when `before` doesn't compile, and
/// [`VerifyError::TooManyGlyphs`] past 65,534 glyphs.
pub fn verify(before: &str, after: &str, options: &VerifyOptions) -> Result<Outcome, VerifyError> {
    let order = glyph_order(before, after, options.glyphs);
    if order.len() > (LAST_CODE_POINT - FIRST_CODE_POINT) as usize {
        return Err(VerifyError::TooManyGlyphs(order.len()));
    }
    let map = GlyphMap::new(order.iter().map(|n| GlyphIdent::Name(n.as_str().into())))
        .expect("names are unique");

    let old = compile(before, &map, options.path).map_err(VerifyError::SourceDoesNotCompile)?;
    let new = match compile(after, &map, options.path) {
        Ok(new) => new,
        Err(errors) => return Ok(Outcome::Breaks { errors }),
    };
    let (old_font, new_font) = (stand_in(&old, order.len()), stand_in(&new, order.len()));
    if old_font == new_font {
        return Ok(Outcome::SameTables);
    }

    let touched = touched_glyphs(before, after, &map);
    let mut systems = language_systems(&old);
    systems.extend(language_systems(&new));
    let tags: BTreeSet<Tag> = feature_tags(&old).chain(feature_tags(&new)).collect();
    // every feature at once, for how they interact; then each alone, since a
    // feature's tag decides when it applies (renaming one changes nothing
    // while all are on)
    let all_on: Vec<_> = tags
        .iter()
        .map(|&t| harfrust::Feature::new(t, 1, ..))
        .collect();
    let mut settings = vec![(None, all_on)];
    for &only in &tags {
        let features = tags
            .iter()
            .map(|&t| harfrust::Feature::new(t, u32::from(t == only), ..))
            .collect();
        settings.push((Some(only.to_string()), features));
    }

    let sequences = sequences(&touched);
    let old_ref = FontRef::new(&old_font).expect("stand-in font parses");
    let new_ref = FontRef::new(&new_font).expect("stand-in font parses");
    let (old_data, new_data) = (ShaperData::new(&old_ref), ShaperData::new(&new_ref));
    let (old_shaper, new_shaper) = (
        old_data.shaper(&old_ref).build(),
        new_data.shaper(&new_ref).build(),
    );
    let mut shaped = 0;
    for (script, language) in &systems {
        let lang = language_override(script, language.as_deref());
        for (feature, features) in &settings {
            // each feature alone gets the singles and pairs; all together get everything
            let tried = if feature.is_some() {
                &sequences[..sequences.len().min(2 * SAMPLE)]
            } else {
                &sequences[..]
            };
            for seq in tried {
                let a = shape(&old_shaper, seq, &lang, features);
                let b = shape(&new_shaper, seq, &lang, features);
                if a != b {
                    let name = |gid: u32| order[gid as usize].clone();
                    return Ok(Outcome::Differs(Difference {
                        input: seq.iter().map(|&g| name(g)).collect(),
                        script: script.trim_end().to_string(),
                        language: language.as_ref().map(|l| l.trim_end().to_string()),
                        feature: feature.as_ref().map(|f| f.trim_end().to_string()),
                        before: describe(&a, &name),
                        after: describe(&b, &name),
                    }));
                }
            }
            shaped += tried.len();
        }
    }
    Ok(Outcome::SameBehaviour { sequences: shaped })
}

/// `.notdef`, then every glyph either version or the font names.
fn glyph_order(before: &str, after: &str, font: Option<&GlyphNames>) -> Vec<String> {
    let mut names = BTreeSet::new();
    for text in [before, after] {
        // hyphenated words, so fea-rs reads `lam-ar` as a glyph, not a range
        names.extend(crate::fmt::file_glyph_names(text, true));
        names.extend(
            symbols(text)
                .iter()
                .filter(|s| s.kind == SymbolKind::Glyph)
                .map(|s| s.name.trim_start_matches('\\').to_string()),
        );
    }
    if let Some(font) = font {
        names.extend(font.iter().map(str::to_string));
    }
    names.remove(".notdef");
    std::iter::once(".notdef".to_string())
        .chain(names)
        .collect()
}

fn compile(text: &str, map: &GlyphMap, path: Option<&Path>) -> Result<Compilation, String> {
    let root = path.map_or_else(|| Path::new("<memory>").to_path_buf(), Path::to_path_buf);
    let base = root.parent().map(Path::to_path_buf);
    let text: Arc<str> = text.into();
    let root_name = root.clone();
    let resolver = move |p: &Path| {
        if p == root_name {
            return Ok(text.clone());
        }
        let file = match &base {
            Some(base) if p.is_relative() => base.join(p),
            _ => p.to_path_buf(),
        };
        std::fs::read_to_string(&file)
            .map(Into::into)
            .map_err(|e| fea_rs::parse::SourceLoadError::new(p.to_path_buf(), e))
    };
    let (tree, diagnostics) = fea_rs::parse::parse_root(root, Some(map), Box::new(resolver))
        .map_err(|e| e.to_string())?;
    if diagnostics.has_errors() {
        return Err(diagnostics.display().to_string());
    }
    fea_rs::compile::compile::<NopVariationInfo, NopFeatureProvider>(
        &tree,
        map,
        None,
        None,
        Opts::new(),
    )
    .map(|(compilation, _warnings)| compilation)
    .map_err(|errors| errors.display().to_string())
}

/// The compiled tables in a font of box glyphs, each mapped to a code point.
fn stand_in(compilation: &Compilation, glyphs: usize) -> Vec<u8> {
    let mut builder: FontBuilder = compilation
        .to_font_builder()
        .expect("compiled tables build");
    let mappings = (1..glyphs).map(|gid| {
        let c = char::from_u32(FIRST_CODE_POINT + gid as u32).expect("private-use code point");
        (c, GlyphId::new(gid as u32))
    });
    let count = u16::try_from(glyphs).expect("checked against the code point range");
    builder
        .add_table(&Cmap::from_mappings(mappings).expect("one glyph per code point"))
        .and_then(|b| {
            b.add_table(&Head {
                units_per_em: 1000,
                ..Default::default()
            })
        })
        .and_then(|b| {
            b.add_table(&Hhea {
                ascender: 800.into(),
                descender: (-200).into(),
                number_of_h_metrics: count,
                ..Default::default()
            })
        })
        .and_then(|b| {
            b.add_table(&Hmtx::new(
                vec![LongMetric::new(ADVANCE, 0); glyphs],
                vec![],
            ))
        })
        .and_then(|b| b.add_table(&Maxp::new(count)))
        .expect("stand-in tables build");
    builder.build()
}

/// Every script and language the font registers, as `(script, language)`;
/// `None` is the script's default language.
fn language_systems(compilation: &Compilation) -> BTreeSet<(String, Option<String>)> {
    let mut out = BTreeSet::new();
    let lists = [
        compilation.gsub.as_ref().map(|t| &*t.script_list),
        compilation.gpos.as_ref().map(|t| &*t.script_list),
    ];
    for list in lists.into_iter().flatten() {
        for record in &list.script_records {
            let script = record.script_tag.to_string();
            out.insert((script.clone(), None));
            for lang in &record.script.lang_sys_records {
                out.insert((script.clone(), Some(lang.lang_sys_tag.to_string())));
            }
        }
    }
    if out.is_empty() {
        out.insert(("DFLT".to_string(), None));
    }
    out
}

fn feature_tags(compilation: &Compilation) -> impl Iterator<Item = Tag> + '_ {
    let gsub = compilation
        .gsub
        .iter()
        .flat_map(|t| &t.feature_list.feature_records);
    let gpos = compilation
        .gpos
        .iter()
        .flat_map(|t| &t.feature_list.feature_records);
    gsub.chain(gpos).map(|r| r.feature_tag)
}

/// HarfBuzz's private-use language tag that selects exact OpenType script and
/// language tags, so every system the file registers can be shaped.
fn language_override(script: &str, language: Option<&str>) -> harfrust::Language {
    let mut tag = format!("x-hbsc{}", script.trim_end());
    if let Some(language) = language {
        tag.push_str("-hbot");
        tag.push_str(language.trim_end());
    }
    tag.parse().expect("a valid BCP 47 private-use tag")
}

/// Glyph IDs written in the part of the text that differs, in either
/// version, with the members of classes named there.
fn touched_glyphs(before: &str, after: &str, map: &GlyphMap) -> Vec<u32> {
    let (old_range, new_range) = changed_ranges(before, after);
    let mut names = BTreeSet::new();
    for (text, range) in [(before, old_range), (after, new_range)] {
        let symbols = symbols(text);
        let overlaps = |r: &Range<usize>| r.start < range.end && range.start < r.end;
        let mut classes = Vec::new();
        for symbol in symbols.iter() {
            if !symbol.occurrences().any(overlaps) {
                continue;
            }
            match symbol.kind {
                SymbolKind::Glyph => {
                    names.insert(symbol.name.trim_start_matches('\\').to_string());
                }
                SymbolKind::Class | SymbolKind::MarkClass => classes.push(symbol.name.clone()),
                _ => {}
            }
        }
        names.extend(class_members(text, &symbols, classes));
    }
    names
        .iter()
        .filter_map(|n| map.get(n.as_str()))
        .map(|gid| gid.to_u32())
        .filter(|&gid| gid != 0)
        .collect()
}

/// The glyphs in the definitions of `classes`, following classes named in them.
fn class_members(
    text: &str,
    symbols: &crate::Symbols,
    mut classes: Vec<String>,
) -> BTreeSet<String> {
    let glyphs: Vec<_> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Glyph)
        .collect();
    let all_classes: Vec<_> = symbols
        .iter()
        .filter(|s| matches!(s.kind, SymbolKind::Class | SymbolKind::MarkClass))
        .collect();
    let mut seen = BTreeSet::new();
    let mut out = BTreeSet::new();
    while let Some(name) = classes.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(class) = all_classes.iter().find(|s| s.name == name) else {
            continue;
        };
        for def in &class.definitions {
            // the statement runs from the name to its `;`
            let end = text[def.end..]
                .find(';')
                .map_or(text.len(), |i| def.end + i);
            let statement = def.start..end;
            let inside = |r: &Range<usize>| statement.start <= r.start && r.end <= statement.end;
            for g in &glyphs {
                if g.references.iter().any(inside) {
                    out.insert(g.name.trim_start_matches('\\').to_string());
                }
            }
            for c in &all_classes {
                if c.name != name && c.references.iter().any(inside) {
                    classes.push(c.name.clone());
                }
            }
        }
    }
    out
}

/// The byte ranges of `before` and `after` between their common prefix and
/// suffix, widened to whole lines.
fn changed_ranges(before: &str, after: &str) -> (Range<usize>, Range<usize>) {
    let prefix = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let max_suffix = before.len().min(after.len()) - prefix;
    let suffix = before
        .bytes()
        .rev()
        .zip(after.bytes().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count();
    let line_start = |text: &str, i: usize| text[..i].rfind('\n').map_or(0, |n| n + 1);
    let line_end = |text: &str, i: usize| text[i..].find('\n').map_or(text.len(), |n| i + n);
    let start_old = line_start(before, prefix);
    let start_new = line_start(after, prefix);
    (
        start_old..line_end(before, before.len() - suffix),
        start_new..line_end(after, after.len() - suffix),
    )
}

/// Every glyph alone, every pair (a sample past [`SAMPLE`]), and [`SAMPLE`]
/// random runs of three to six. Seeded, so a result can be reproduced.
fn sequences(glyphs: &[u32]) -> Vec<Vec<u32>> {
    if glyphs.is_empty() {
        return Vec::new();
    }
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let mut out: Vec<Vec<u32>> = glyphs.iter().map(|&g| vec![g]).collect();
    let pairs = glyphs.len() * glyphs.len();
    if pairs <= SAMPLE {
        for &a in glyphs {
            out.extend(glyphs.iter().map(|&b| vec![a, b]));
        }
    } else {
        for _ in 0..SAMPLE {
            out.push(vec![rng.pick(glyphs), rng.pick(glyphs)]);
        }
    }
    for _ in 0..SAMPLE {
        let len = 3 + rng.below(4);
        out.push((0..len).map(|_| rng.pick(glyphs)).collect());
    }
    out
}

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick(&mut self, from: &[u32]) -> u32 {
        from[self.below(from.len())]
    }
}

/// A shaped glyph: ID, advances and offsets.
type Shaped = (u32, i32, i32, i32, i32);

fn shape(
    shaper: &harfrust::Shaper,
    glyphs: &[u32],
    language: &harfrust::Language,
    features: &[harfrust::Feature],
) -> Vec<Shaped> {
    let mut buffer = UnicodeBuffer::new();
    for (i, &g) in glyphs.iter().enumerate() {
        let c = char::from_u32(FIRST_CODE_POINT + g).expect("private-use code point");
        buffer.add(c, i as u32);
    }
    buffer.set_direction(harfrust::Direction::LeftToRight);
    buffer.set_script(harfrust::script::COMMON);
    buffer.set_language(language.clone());
    let out = shaper.shape(buffer, harfrust::ShapeOptions::new().features(features));
    out.glyph_infos()
        .iter()
        .zip(out.glyph_positions())
        .map(|(i, p)| (i.glyph_id, p.x_advance, p.y_advance, p.x_offset, p.y_offset))
        .collect()
}

/// Glyph names, with `@x,y` for an offset and `+advance` when it isn't the default.
fn describe(shaped: &[Shaped], name: &impl Fn(u32) -> String) -> Vec<String> {
    shaped
        .iter()
        .map(|&(gid, x_advance, y_advance, x_offset, y_offset)| {
            let mut s = name(gid);
            if x_offset != 0 || y_offset != 0 {
                s.push_str(&format!("@{x_offset},{y_offset}"));
            }
            if x_advance != i32::from(ADVANCE) || y_advance != 0 {
                s.push_str(&format!("+{x_advance}"));
            }
            s
        })
        .collect()
}
