//! `--verify`: keep only changes that don't alter what the font does.

use std::path::Path;

use feanor::verify::{Difference, Outcome, VerifyOptions, verify};
use feanor::{Applicability, Diagnostic, GlyphNames, apply_fixes};

/// Why a change wasn't verified.
pub(crate) enum Rejected {
    /// The original doesn't compile, so nothing can be compared.
    Unverifiable(String),
    /// The change makes the font do something else.
    Changes(String),
}

/// Whether `after` makes the font do what `before` does.
pub(crate) fn check(
    before: &str,
    after: &str,
    path: &Path,
    glyphs: Option<&GlyphNames>,
) -> Result<(), Rejected> {
    let options = VerifyOptions {
        glyphs,
        path: Some(path),
    };
    match verify(before, after, &options) {
        Ok(Outcome::SameTables | Outcome::SameBehaviour { .. }) => Ok(()),
        Ok(Outcome::Differs(d)) => Err(Rejected::Changes(describe(&d))),
        Ok(Outcome::Breaks { errors }) => Err(Rejected::Changes(format!(
            "the result doesn't compile:\n{}",
            errors.trim_end()
        ))),
        Ok(_) => Err(Rejected::Changes("an unknown outcome".into())),
        Err(e) => Err(Rejected::Unverifiable(e.to_string())),
    }
}

fn describe(d: &Difference) -> String {
    let mut context = vec![format!("script {}", d.script)];
    context.extend(d.language.as_ref().map(|l| format!("language {l}")));
    context.push(match &d.feature {
        Some(f) => format!("only {f} on"),
        None => "all features on".to_string(),
    });
    format!(
        "{} becomes {} instead of {} ({})",
        d.input.join(" "),
        d.after.join(" "),
        d.before.join(" "),
        context.join(", ")
    )
}

/// The fixed text, applying only fixes that verify, and how many were left
/// out. Tries all of them at once first; when that changes the font, each fix
/// alone, then those that pass together. Prints what it left out.
pub(crate) fn fix(
    text: &str,
    diags: &[Diagnostic],
    allowed: Applicability,
    path: &Path,
    glyphs: Option<&GlyphNames>,
) -> (String, usize) {
    let all = apply_fixes(text, diags, allowed);
    if all == text {
        return (all, 0);
    }
    let shown = path.display();
    match check(text, &all, path, glyphs) {
        Ok(()) => return (all, 0),
        Err(Rejected::Unverifiable(why)) => {
            eprintln!("{shown}: not fixed: can't verify, {why}");
            return (text.to_string(), 1);
        }
        Err(Rejected::Changes(_)) => {}
    }
    let mut kept = Vec::new();
    let mut refused = 0;
    for d in diags {
        let one = apply_fixes(text, std::slice::from_ref(d), allowed);
        if one == text {
            continue;
        }
        match check(text, &one, path, glyphs) {
            Ok(()) => kept.push(d.clone()),
            Err(Rejected::Changes(why) | Rejected::Unverifiable(why)) => {
                refused += 1;
                eprintln!(
                    "{shown}: not fixed: {} {} would change the font: {why}",
                    d.rule.code(),
                    d.rule
                );
            }
        }
    }
    let fixed = apply_fixes(text, &kept, allowed);
    match check(text, &fixed, path, glyphs) {
        Ok(()) => (fixed, refused),
        Err(Rejected::Changes(why) | Rejected::Unverifiable(why)) => {
            eprintln!("{shown}: not fixed: the fixes change the font together: {why}");
            (text.to_string(), refused.max(1))
        }
    }
}
