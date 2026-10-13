//! Reading feature files and glyph lists, with errors a person can act on.

use std::path::Path;

use fea_tools::GlyphNames;

pub(crate) struct Input {
    pub text: String,
}

/// Read a feature file as UTF-8. On failure, print why and return `None`, so
/// one bad file doesn't stop a run over many.
pub(crate) fn read_input(path: &Path) -> Option<Input> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {}: {e}", path.display());
            return None;
        }
    };
    match String::from_utf8(bytes) {
        Ok(text) => Some(Input { text }),
        Err(e) => {
            let at = e.utf8_error().valid_up_to();
            eprintln!(
                "error: {}: not UTF-8 (invalid byte at offset {at}); feature files must be UTF-8. Convert it first, e.g. `iconv -f latin1 -t utf-8`",
                path.display()
            );
            None
        }
    }
}

/// A glyph list: a UFO (its `glyphs/contents.plist`) or a text file with one name per line.
pub(crate) fn load_glyphs(path: &Path) -> Result<GlyphNames, String> {
    if path.is_dir() {
        let plist = path.join("glyphs/contents.plist");
        return match plist::Value::from_file(&plist) {
            Ok(plist::Value::Dictionary(d)) => Ok(d.keys().cloned().collect()),
            Ok(_) => Err(format!(
                "{}: expected a dictionary of glyph names",
                plist.display()
            )),
            Err(e) => Err(format!("{}: {e}", plist.display())),
        };
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect())
}
