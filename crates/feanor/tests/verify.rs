//! Comparing what two versions of a feature file make a font do.

use feanor::GlyphNames;
use feanor::verify::{Outcome, VerifyError, VerifyOptions, verify};

fn check(before: &str, after: &str) -> Outcome {
    verify(before, after, &VerifyOptions::default()).expect("the original compiles")
}

#[test]
fn layout_changes_compile_to_the_same_tables() {
    let before = "feature liga {sub f i by f_i;\nsub f l by f_l;} liga;\n";
    let after = "feature liga {\n    sub f i by f_i;\n    sub f l by f_l;\n} liga;\n";
    assert_eq!(check(before, after), Outcome::SameTables);
}

#[test]
fn reordered_rules_change_what_a_sequence_becomes() {
    let before = "feature calt {\n    sub a' c by a.short;\n    sub a' by a.alt;\n} calt;\n";
    let after = "feature calt {\n    sub a' by a.alt;\n    sub a' c by a.short;\n} calt;\n";
    let Outcome::Differs(d) = check(before, after) else {
        panic!("expected a difference");
    };
    assert_eq!(d.input, ["a", "c"]);
    assert_eq!(d.before, ["a.short", "c"]);
    assert_eq!(d.after, ["a.alt", "c"]);
}

#[test]
fn renaming_a_feature_changes_when_it_applies() {
    let before = "feature smpc {\n    sub a by a.sc;\n} smpc;\n";
    let after = "feature smcp {\n    sub a by a.sc;\n} smcp;\n";
    let Outcome::Differs(d) = check(before, after) else {
        panic!("expected a difference");
    };
    assert_eq!(d.input, ["a"]);
    assert!(matches!(d.feature.as_deref(), Some("smcp" | "smpc")));
}

#[test]
fn removing_an_ignored_duplicate_pair_changes_nothing() {
    let before = "feature kern {\n    pos T o -60;\n    pos T o -45;\n} kern;\n";
    let after = "feature kern {\n    pos T o -60;\n} kern;\n";
    assert!(matches!(
        check(before, after),
        Outcome::SameTables | Outcome::SameBehaviour { .. }
    ));
}

#[test]
fn a_changed_kerning_value_is_a_difference_in_position() {
    let before = "feature kern {\n    pos T o -60;\n} kern;\n";
    let after = "feature kern {\n    pos T o -45;\n} kern;\n";
    let Outcome::Differs(d) = check(before, after) else {
        panic!("expected a difference");
    };
    assert_eq!(d.input, ["T", "o"]);
    assert_eq!(d.before, ["T+440", "o"]);
    assert_eq!(d.after, ["T+455", "o"]);
}

#[test]
fn a_feature_registered_for_one_script_is_shaped_under_it() {
    let before = "languagesystem arab dflt;\nfeature calt {\n    sub beh by beh.alt;\n} calt;\n";
    let after = "languagesystem arab dflt;\nfeature calt {\n    sub beh by beh.alt2;\n} calt;\n";
    let Outcome::Differs(d) = check(before, after) else {
        panic!("expected a difference");
    };
    assert_eq!(d.script, "arab");
    assert_eq!(d.before, ["beh.alt"]);
    assert_eq!(d.after, ["beh.alt2"]);
}

#[test]
fn a_duplicate_removed_from_a_class_paired_by_index_breaks_the_rule() {
    // what C002's fix did in 0.3.0
    let before =
        "@from = [a b a];\n@to = [x y z];\nfeature ss01 {\n    sub @from by @to;\n} ss01;\n";
    let after = "@from = [a b];\n@to = [x y z];\nfeature ss01 {\n    sub @from by @to;\n} ss01;\n";
    assert!(matches!(check(before, after), Outcome::Breaks { .. }));
}

#[test]
fn verifying_uses_fea_rs_so_a_stray_backslash_it_ignores_passes() {
    // the other C002 bug in 0.3.0 left `\` behind. fea-rs ignores it and
    // compiles the same tables; fontTools rejects the file. Verifying can only
    // speak for fea-rs, so catching this is a lint rule's job.
    let before = "@c = [\\a b \\a];\nfeature ss01 {\n    sub @c by x;\n} ss01;\n";
    let after = "@c = [\\a b \\];\nfeature ss01 {\n    sub @c by x;\n} ss01;\n";
    assert_eq!(check(before, after), Outcome::SameTables);
}

#[test]
fn removing_a_plain_duplicate_from_a_class_changes_nothing() {
    let before = "@c = [a b a];\nfeature ss01 {\n    sub @c by x;\n} ss01;\n";
    let after = "@c = [a b];\nfeature ss01 {\n    sub @c by x;\n} ss01;\n";
    assert!(matches!(
        check(before, after),
        Outcome::SameTables | Outcome::SameBehaviour { .. }
    ));
}

#[test]
fn a_change_to_a_class_is_tested_on_its_members() {
    let before = "@lc = [a b c];\nfeature smcp {\n    sub @lc by a.sc;\n} smcp;\n";
    let after = "@lc = [a b];\nfeature smcp {\n    sub @lc by a.sc;\n} smcp;\n";
    let Outcome::Differs(d) = check(before, after) else {
        panic!("expected a difference");
    };
    assert_eq!(d.input, ["c"]);
}

#[test]
fn the_font_glyph_list_lets_ranges_compile() {
    let glyphs: GlyphNames = ["a", "b", "c", "a.sc", "b.sc", "c.sc"]
        .into_iter()
        .collect();
    let options = VerifyOptions {
        glyphs: Some(&glyphs),
        ..Default::default()
    };
    let before = "feature smcp {\n    sub [a - c] by [a.sc - c.sc];\n} smcp;\n";
    let after = "feature smcp {\n    sub [a - b] by [a.sc - b.sc];\n} smcp;\n";
    let Ok(Outcome::Differs(d)) = verify(before, after, &options) else {
        panic!("expected a difference");
    };
    assert_eq!(d.before, ["c.sc"]);
}

#[test]
fn an_original_that_does_not_compile_cannot_be_verified() {
    let result = verify(
        "feature smcp { sub a by; } smcp;",
        "",
        &VerifyOptions::default(),
    );
    assert!(matches!(result, Err(VerifyError::SourceDoesNotCompile(_))));
}
