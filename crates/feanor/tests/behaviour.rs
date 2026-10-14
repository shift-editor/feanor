//! Behaviour tests: what the formatter prints, what the linter reports, and
//! that the incremental linter always agrees with a full lint.

use std::path::Path;

use feanor::internal::{first_token_mismatch, lint_whole_document};
use feanor::{
    Applicability, Diagnostic, FormatOptions, GlyphNames, Incremental, Indent, LintOptions,
    Project, Rule, Severity, apply_noqa, detect_indent, format, lint,
};

fn four_spaces() -> FormatOptions {
    let mut options = FormatOptions::default();
    options.indent = Indent::Spaces(4);
    options
}

fn fmt(src: &str) -> String {
    format(src, &four_spaces()).text
}

/// Formatting must be idempotent and keep every token and comment.
fn assert_safe(src: &str) {
    let once = format(src, &four_spaces()).text;
    let twice = format(&once, &four_spaces()).text;
    assert_eq!(once, twice, "formatting is not idempotent");
    assert_eq!(
        first_token_mismatch(src, &once),
        None,
        "formatting changed a token"
    );
}

fn glyphs(names: &[&str]) -> GlyphNames {
    names.iter().copied().collect()
}

fn rules(diags: &[Diagnostic]) -> Vec<Rule> {
    diags.iter().map(|d| d.rule).collect()
}

fn plain() -> LintOptions<'static> {
    LintOptions::default()
}

// ------------------------------------------------------------------ formatter

#[test]
fn packs_a_long_class_into_a_block_and_keeps_comments_in_place() {
    let src = "@lc = [a b   c\n  # accented\n  aacute   agrave];\n";
    assert_eq!(
        fmt(src),
        "@lc = [\n    a b c\n    # accented\n    aacute agrave\n];\n"
    );
    assert_safe(src);
}

#[test]
fn indents_blocks_and_keeps_trailing_comments() {
    let src = "feature liga {\nsub f i by fi; # trailing\n} liga;\n";
    assert_eq!(
        fmt(src),
        "feature liga {\n    sub f i by fi; # trailing\n} liga;\n"
    );
}

#[test]
fn keeps_a_block_on_one_line_only_when_the_author_did() {
    assert_eq!(
        fmt("feature kern { pos a b -10; } kern;\n"),
        "feature kern { pos a b -10; } kern;\n"
    );
    assert_eq!(
        fmt("feature kern {\npos a b -10; } kern;\n"),
        "feature kern {\n    pos a b -10;\n} kern;\n"
    );
}

#[test]
fn wraps_a_long_rule_before_by() {
    let src = format!(
        "feature calt {{\n    sub {}' x by a_very_long_ligature_name;\n}} calt;\n",
        "glyphname ".repeat(8).trim_end()
    );
    let out = fmt(&src);
    assert!(
        out.contains("x\n        by a_very_long_ligature_name;"),
        "{out}"
    );
    assert_safe(&src);
}

#[test]
fn wraps_a_long_run_of_glyphs_between_them() {
    let src = format!(
        "feature calt {{\n    sub {}' x by y;\n}} calt;\n",
        "glyphname ".repeat(10).trim_end()
    );
    assert_eq!(
        fmt(&src),
        "feature calt {\n    sub glyphname glyphname glyphname glyphname glyphname glyphname glyphname \
         glyphname glyphname\n        glyphname' x by y;\n} calt;\n"
    );
    assert_safe(&src);
}

#[test]
fn puts_each_class_of_a_long_contextual_rule_on_its_own_line() {
    let class = "[@lowerVoc0 @lowerVoc1 @lowerVoc2 @lowerEtc]";
    let src = format!(
        "feature calt {{\n    sub @lowerCon0 {class} {class} @lowerCon0' by @lowerCon1;\n}} calt;\n"
    );
    assert_eq!(
        fmt(&src),
        format!(
            "feature calt {{\n    sub @lowerCon0\n        {class}\n        {class}\n        \
             @lowerCon0' by @lowerCon1;\n}} calt;\n"
        )
    );
    assert_safe(&src);
}

#[test]
fn splits_a_class_only_when_its_own_line_is_too_long() {
    let long = format!("[{}]", "glyphname.alt ".repeat(9).trim_end());
    let src = format!("feature calt {{\n    sub a {long} b' by c;\n}} calt;\n");
    let out = fmt(&src);
    assert!(out.contains("    sub a [\n"), "{out}");
    assert!(out.lines().all(|l| l.chars().count() <= 100), "{out}");
    assert_safe(&src);
}

#[test]
fn leaves_fea_fmt_off_regions_exactly_as_written() {
    let src = "feature ss05 {\n    # fmt: off\n    sub A      by A.circled;\n    # fmt: on\n    sub B    by B.circled;\n} ss05;\n";
    let out = fmt(src);
    assert!(out.contains("    sub A      by A.circled;\n"));
    assert!(out.contains("    sub B by B.circled;\n"));
}

#[test]
fn realigns_runs_the_author_aligned_and_flattens_the_rest() {
    // rules alone aren't valid at the top level, so these are formatted as a feature body
    let aligned = "sub A        by A.circled;\nsub BB by B.circled;\n";
    assert_eq!(fmt(aligned), "sub A  by A.circled;\nsub BB by B.circled;\n");
    let plain = "sub A by A.circled;\nsub BB by B.circled;\n";
    assert_eq!(fmt(plain), plain);
    assert_safe(aligned);
}

#[test]
fn keeps_class_tables_as_tables() {
    let src = "@a = [ zero      one ];\n@b = [ zerosuperior onesuperior ];\n";
    assert_eq!(
        fmt(src),
        "@a = [zero         one];\n@b = [zerosuperior onesuperior];\n"
    );
    assert_safe(src);
}

// A class that starts on a new line after `[` keeps the author's line breaks;
// a class written on one line is packed to the line width.

fn narrow(width: usize) -> FormatOptions {
    let mut options = four_spaces();
    options.line_width = width;
    options
}

#[test]
fn keeps_the_rows_of_a_class_that_starts_on_a_new_line() {
    let src = "@vowels = [\n  a   aacute agrave\n  e eacute\n  i\n];\n";
    assert_eq!(
        fmt(src),
        "@vowels = [\n    a aacute agrave\n    e eacute\n    i\n];\n"
    );
    assert_safe(src);
}

#[test]
fn keeps_a_class_written_one_glyph_per_line() {
    let src = "@figures = [\n    zero\n    one\n    two\n];\n";
    assert_eq!(fmt(src), src);
    assert_safe(src);
}

#[test]
fn packs_a_class_written_on_one_line() {
    assert_eq!(fmt("@abc = [a   b c];\n"), "@abc = [a b c];\n");
    let src = "@greek = [alpha beta gamma delta epsilon zeta];\n";
    assert_eq!(
        format(src, &narrow(30)).text,
        "@greek = [\n    alpha beta gamma delta\n    epsilon zeta\n];\n"
    );
}

#[test]
fn packs_a_class_that_wraps_without_a_break_after_the_bracket() {
    // as FontLab exports them: a long line wrapped at a fixed column
    let src = "@lc = [a b c\n\td e\n\tf];\n";
    assert_eq!(fmt(src), "@lc = [a b c d e f];\n");
    assert_safe(src);
}

#[test]
fn wraps_only_the_row_that_is_too_long() {
    let src = "@greek = [\n    alpha beta gamma delta epsilon\n    zeta eta\n];\n";
    let out = format(src, &narrow(30)).text;
    assert_eq!(
        out,
        "@greek = [\n    alpha beta gamma delta\n    epsilon\n    zeta eta\n];\n"
    );
    assert_eq!(format(&out, &narrow(30)).text, out, "not idempotent");
}

#[test]
fn keeps_blank_lines_and_comments_in_a_class_that_keeps_its_rows() {
    let src = "@marks = [ # above\n  acutecomb gravecomb\n\n\n  # below\n  dotbelowcomb\n];\n";
    assert_eq!(
        fmt(src),
        "@marks = [ # above\n    acutecomb gravecomb\n\n    # below\n    dotbelowcomb\n];\n"
    );
    assert_safe(src);
}

#[test]
fn keeps_parallel_classes_row_by_row() {
    let src = "@lc = [\n    a b c\n    d e\n];\n@sc = [\n    a.sc b.sc c.sc\n    d.sc e.sc\n];\n\nfeature smcp {\n    sub @lc by @sc;\n} smcp;\n";
    assert_eq!(fmt(src), src);
    assert_safe(src);
}

#[test]
fn reindents_the_rows_it_keeps() {
    let src = "@lc = [\n\ta b\n\t\tc\n];\n";
    assert_eq!(fmt(src), "@lc = [\n    a b\n    c\n];\n");
    assert_safe(src);
}

#[test]
fn leaves_classes_it_already_packed_alone() {
    // output of the packing rule starts a new line after `[`, so a second
    // pass keeps those rows: files formatted before don't change
    let src = "@greek = [alpha beta gamma delta epsilon zeta eta theta];\n";
    let once = format(src, &narrow(30)).text;
    assert_eq!(format(&once, &narrow(30)).text, once);
    assert_eq!(
        format(&once, &narrow(100)).text,
        once,
        "a wider limit repacks a kept class"
    );
}

#[test]
fn keeps_rows_in_a_class_inside_a_rule() {
    let src = "feature liga {\n    sub [\n        f\n        f_f\n    ] i by fi;\n} liga;\n";
    assert_eq!(fmt(src), src);
    assert_safe(src);
}

#[test]
fn does_not_put_a_block_on_one_line_when_a_class_keeps_its_rows() {
    let src = "feature liga { sub [\n f\n f_f\n] i by fi; } liga;\n";
    let out = fmt(src);
    assert!(out.contains("[\n        f\n        f_f\n    ]"), "{out}");
    assert_safe(src);
}

#[test]
fn a_table_with_nothing_to_align_is_formatted_as_usual() {
    // Roboto in fea-rs's test data: a stray double space made two long, identical
    // classes look like a table; kept on one line, the next pass wrapped them
    let long = "S Sacute Scircumflex Scedilla Scaron uni0218 uni2C7E uni1E60 uni1E62 uni1E64 uni1E66 uni1E68";
    let src = format!(
        "@_S_L = [{}];\n@_S_R = [{}];\n",
        long.replacen(' ', "  ", 1),
        long.replacen(' ', "  ", 1)
    );
    let once = format(&src, &narrow(60)).text;
    assert_eq!(format(&once, &narrow(60)).text, once, "not idempotent");
    assert!(once.lines().all(|l| l.chars().count() <= 60), "{once}");
}

#[test]
fn leaves_unparseable_items_untouched() {
    let src =
        "feature liga {\n  sub a by b;\n} liga;\n@broken = [a b\nfeature kern {pos a b 1;} kern;\n";
    let out = format(src, &four_spaces());
    assert!(!out.untouched.is_empty());
    assert_safe(src);
}

#[test]
fn formats_feature_bodies_without_a_wrapper() {
    let out = format("sub a   by b;\n  sub c by d;\n", &four_spaces());
    assert!(out.feature_body);
    assert_eq!(out.text, "sub a by b;\nsub c by d;\n");
}

#[test]
fn detects_the_files_indent_unit() {
    assert_eq!(
        detect_indent("feature a {\n  sub a by b;\n} a;\n").as_deref(),
        Some("  ")
    );
    assert_eq!(
        detect_indent("feature a {\n\tsub a by b;\n} a;\n").as_deref(),
        Some("\t")
    );
    assert_eq!(detect_indent("@a = [\n  b c\n];\n").as_deref(), Some("  "));
    assert_eq!(detect_indent("@a = [b];\n"), None);
    // uneven nesting (3, then 8) doesn't make the unit 5
    let uneven = "feature frac {\n   lookup A {\n        sub a by b;\n   } A;\n   lookup B {\n        sub c by d;\n   } B;\n} frac;\n";
    assert_eq!(detect_indent(uneven).as_deref(), Some("   "));
}

#[test]
fn formats_with_a_configured_or_detected_indent() {
    let mut tab = FormatOptions::default();
    tab.indent = Indent::Tab;
    assert_eq!(
        format("feature a {\nsub a by b;\n} a;\n", &tab).text,
        "feature a {\n\tsub a by b;\n} a;\n"
    );
    let two = "feature a {\n  sub a by b;\n  sub c by d;\n} a;\n";
    assert_eq!(format(two, &FormatOptions::default()).text, two);
}

// ------------------------------------------------------------------ linter

const DEMO: &str = "languagesystem DFLT dflt;
@lc = [a b c b];
@sc = [a.sc b.sc];
@unused = [a];
lookup Never { sub a by b; } Never;
feature smpc {
    sub @lc by @sc;
    sub agrve by a.sc;
    sub a\u{a0}b by c;
} smpc;
";

fn demo_font() -> GlyphNames {
    glyphs(&["a", "b", "c", "a.sc", "b.sc", "c.sc", "agrave"])
}

#[test]
fn every_rule_fires_on_the_demo() {
    let font = demo_font();
    let diags = lint(DEMO, &LintOptions::default().with_glyphs(&font));
    let got = rules(&diags);
    for rule in [
        Rule::DuplicateGlyphInClass,
        Rule::UnusedClass,
        Rule::UnusedLookup,
        Rule::UnknownFeatureTag,
        Rule::ClassLengthMismatch,
        Rule::GlyphNotInFont,
        Rule::InvisibleCharacter,
    ] {
        assert!(got.contains(&rule), "{rule} missing from {got:?}");
    }
    let find = |rule| diags.iter().find(|d| d.rule == rule).unwrap();
    assert!(
        find(Rule::GlyphNotInFont)
            .message
            .contains("did you mean 'agrave'")
    );
    let mismatch = &find(Rule::ClassLengthMismatch).message;
    assert!(mismatch.contains("@sc is missing c.sc"), "{mismatch}");
}

#[test]
fn renames_are_unsafe_and_cleanups_are_safe() {
    let font = demo_font();
    let diags = lint(DEMO, &LintOptions::default().with_glyphs(&font));
    let applicability =
        |rule| diags.iter().find(|d| d.rule == rule).unwrap().fixes[0].applicability;
    assert_eq!(
        applicability(Rule::DuplicateGlyphInClass),
        Applicability::Safe
    );
    assert_eq!(applicability(Rule::InvisibleCharacter), Applicability::Safe);
    assert_eq!(applicability(Rule::GlyphNotInFont), Applicability::Unsafe);
    assert_eq!(
        applicability(Rule::UnknownFeatureTag),
        Applicability::Unsafe
    );
}

#[test]
fn duplicates_on_the_replacement_side_are_allowed() {
    let src = "feature ss01 { sub [Alphatonos Alpha] by [Alpha Alpha]; } ss01;\n";
    assert!(!rules(&lint(src, &plain())).contains(&Rule::DuplicateGlyphInClass));
}

#[test]
fn unknown_tags_in_aalt_references_are_reported() {
    let src = "feature aalt {\n    feature smcp;\n    feature zzzx;\n} aalt;\n";
    let diags = lint(src, &plain());
    let tags: Vec<&str> = diags
        .iter()
        .filter(|d| d.rule == Rule::UnknownFeatureTag)
        .map(|d| &src[d.range.clone()])
        .collect();
    assert_eq!(tags, ["zzzx"]);
}

#[test]
fn class_lengths_count_glyph_ranges() {
    let src = "feature smcp { sub [a - c] by [a.sc b.sc]; } smcp;\n";
    let diags = lint(src, &plain());
    assert!(
        rules(&diags).contains(&Rule::ClassLengthMismatch),
        "{diags:?}"
    );
}

#[test]
fn noqa_silences_a_line_or_named_rules() {
    let src = "@a = [x x]; # noqa: C002\n@b = [y y]; # noqa\n@c = [z z]; # noqa: G001\n";
    let mut diags = lint(src, &plain());
    apply_noqa(src, &mut diags);
    let dups: Vec<_> = diags
        .iter()
        .filter(|d| d.rule == Rule::DuplicateGlyphInClass)
        .collect();
    assert_eq!(dups.len(), 1);
    assert!(src[dups[0].range.clone()].starts_with('z'));
}

#[test]
fn undefined_names_are_errors_only_for_whole_projects() {
    let src = "feature liga { sub @missing by b; lookup Later; } liga;\nlookup Later { sub a by b; } Later;\n";
    let single = rules(&lint(src, &plain()));
    assert!(!single.contains(&Rule::UndefinedClass));
    let project = rules(&lint(src, &plain().whole_project(true)));
    assert!(project.contains(&Rule::UndefinedClass));
    assert!(project.contains(&Rule::UndefinedLookup));
}

#[test]
fn ranges_point_into_feature_bodies() {
    let src = "@a = [x x];\nsub a by b;\n";
    let diags = lint(src, &plain());
    let dup = diags
        .iter()
        .find(|d| d.rule == Rule::DuplicateGlyphInClass)
        .expect("duplicate in a body");
    assert_eq!(&src[dup.range.clone()], "x");
    assert_eq!(
        dup.related.as_ref().map(|r| &src[r.range.clone()]),
        Some("x")
    );
}

// ------------------------------------------------------------------ incremental

#[test]
fn incremental_linting_matches_a_full_lint_after_every_edit() {
    assert_incremental_matches_full_lint(&DEMO.repeat(3));
}

#[test]
fn incremental_linting_matches_a_full_lint_for_the_lookup_rules() {
    assert_incremental_matches_full_lint(&LOOKUP_DEMO.repeat(2));
}

/// 300 random edits; after each, the incremental linter must agree with a full lint.
fn assert_incremental_matches_full_lint(text: &str) {
    let mut doc = Incremental::new(text.to_string(), plain());
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    for step in 0..300 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let len = doc.text().len();
        let mut at = (seed as usize) % len;
        while !doc.text().is_char_boundary(at) {
            at -= 1;
        }
        let insert =
            ["a", " ", ";", "{", "}", "[", "]", "#", "\n", "@"][(seed >> 32) as usize % 10];
        if step % 3 == 0 {
            let mut end = (at + 1).min(len);
            while !doc.text().is_char_boundary(end) {
                end += 1;
            }
            doc.edit(at..end, "");
        } else {
            doc.edit(at..at, insert);
        }
        let key = |ds: Vec<Diagnostic>| {
            ds.into_iter()
                .map(|d| format!("{} {:?} {}", d.rule, d.range, d.message))
                .collect::<Vec<_>>()
        };
        let full = lint_whole_document(doc.text(), &plain());
        assert_eq!(key(doc.diagnostics()), key(full), "after edit {step}");
    }
}

// ------------------------------------------------------------------ lookup rules

/// A document with one of each mistake the lookup rules find.
const LOOKUP_DEMO: &str = "languagesystem DFLT dflt;
markClass acutecomb <anchor 0 500> @TOP;
markClass acutecomb <anchor 0 -10> @BOTTOM;
lookup P { pos a 10; } P;
lookup ALT { sub a by a.alt; } ALT;
lookup EMPTY { } EMPTY;
feature calt {
    sub a' by b;
    sub a' c by d;
    sub e' lookup ALT t;
    sub t' lookup P o;
} calt;
feature ss01 { sub g by g.alt; sub g by g.ss01; } ss01;
feature kern { pos T o -60; pos T o -45; } kern;
feature mark { pos base a <anchor 250 500> mark @TOP; pos base a <anchor 250 0> mark @BOTTOM; } mark;
languagesystem latn dflt;
feature liga { sub f i by f_i;; } liga;
";

fn codes(src: &str) -> Vec<&'static str> {
    lint(src, &plain()).iter().map(|d| d.rule.code()).collect()
}

#[test]
fn every_lookup_rule_fires_on_the_lookup_demo() {
    let got = codes(LOOKUP_DEMO);
    for code in [
        "E002", "E003", "C005", "L003", "R001", "R002", "R003", "R004", "R005",
    ] {
        assert!(got.contains(&code), "{code} missing from {got:?}");
    }
}

#[test]
fn reports_a_contextual_rule_an_earlier_rule_always_matches_first() {
    let src = "feature calt {\n    sub a' by b;\n    sub a' c by d;\n} calt;\n";
    let diags = lint(src, &plain());
    assert_eq!(rules(&diags), [Rule::UnreachableRule]);
    assert_eq!(&src[diags[0].range.clone()], "sub a' c by d;");
    assert_eq!(
        &src[diags[0].related.as_ref().unwrap().range.clone()],
        "sub a' by b;"
    );
}

#[test]
fn a_specific_rule_before_a_general_one_is_fine() {
    assert!(codes("feature calt {\n    sub a' c by d;\n    sub a' by b;\n} calt;\n").is_empty());
}

#[test]
fn an_ignore_rule_can_make_a_later_rule_unreachable() {
    let src = "feature calt {\n    ignore sub a';\n    sub a' c by d;\n} calt;\n";
    assert_eq!(codes(src), ["R001"]);
    // and narrower ignores are the usual way to carve out exceptions
    assert!(codes("feature calt {\n    ignore sub x a';\n    sub a' by b;\n} calt;\n").is_empty());
}

#[test]
fn compares_rules_only_within_one_lookup() {
    // a lookupflag starts a new lookup, so the second rule runs in its own
    let src = "feature calt {\n    sub a' by b;\n    lookupflag IgnoreMarks;\n    sub a' c by d;\n} calt;\n";
    assert!(codes(src).is_empty());
}

#[test]
fn expands_classes_to_find_unreachable_rules() {
    let src = "@V = [a e];\nfeature calt {\n    sub @V' by b;\n    sub e' c by d;\n} calt;\n";
    assert_eq!(codes(src), ["R001"]);
}

#[test]
fn reports_a_glyph_substituted_twice_in_one_lookup() {
    let src = "feature ss01 {\n    sub [a e] by [a.alt e.alt];\n    sub e by e.ss01;\n} ss01;\n";
    let diags = lint(src, &plain());
    assert_eq!(rules(&diags), [Rule::DuplicateSubstitution]);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(
        diags[0].message.starts_with("e is already substituted"),
        "{}",
        diags[0].message
    );
}

#[test]
fn reports_a_ligature_defined_twice() {
    let src = "feature liga {\n    sub f i by f_i;\n    sub f i by fi;\n} liga;\n";
    assert_eq!(codes(src), ["R002"]);
}

#[test]
fn aalt_may_list_one_glyph_several_times() {
    // Open Sans: alternates in aalt are written this way
    let src = "feature aalt {\n    sub Eng by Eng.alt1;\n    sub Eng by Eng.alt2;\n} aalt;\n";
    assert!(codes(src).is_empty());
}

#[test]
fn reports_a_kerning_pair_positioned_twice_and_removes_the_ignored_rule() {
    // Farsan: the second value was silently ignored
    let src = "feature kern {\n    pos gjB gjDha -7;\n    pos gjB gjDha -10;\n} kern;\n";
    let diags = lint(src, &plain());
    assert_eq!(rules(&diags), [Rule::DuplicatePair]);
    assert_eq!(diags[0].fixes[0].applicability, Applicability::Safe);
    let fixed = feanor::apply_fixes(src, &diags, Applicability::Safe);
    assert_eq!(fixed, "feature kern {\n    pos gjB gjDha -7;\n} kern;\n");
}

#[test]
fn class_kerning_pairs_are_not_duplicates_of_glyph_pairs() {
    // glyph pairs take precedence over class pairs by design
    let src = "feature kern {\n    pos [a e] v -10;\n    pos a v -20;\n} kern;\n";
    assert!(codes(src).is_empty());
    // with enum, the class operands become glyph pairs, and a v is then repeated
    let src = "feature kern {\n    enum pos [a e] v -10;\n    pos a v -20;\n} kern;\n";
    assert_eq!(codes(src), ["R003"]);
}

#[test]
fn reports_a_lookup_of_the_wrong_kind() {
    let src = "lookup P { pos a 10; } P;\nfeature calt {\n    sub a' lookup P b;\n} calt;\n";
    let diags = lint(src, &plain());
    assert_eq!(rules(&diags), [Rule::WrongLookupType]);
    assert_eq!(&src[diags[0].range.clone()], "P");
}

#[test]
fn reports_a_lookup_called_on_glyphs_it_has_no_rule_for() {
    let src =
        "lookup ALT { sub a by a.alt; } ALT;\nfeature calt {\n    sub e' lookup ALT t;\n} calt;\n";
    assert_eq!(codes(src), ["R005"]);
    // covering some of the marked glyphs is how such lookups are usually meant
    let src = "lookup ALT { sub a by a.alt; } ALT;\nfeature calt {\n    sub [a e]' lookup ALT t;\n} calt;\n";
    assert!(codes(src).is_empty());
}

#[test]
fn reports_mark_classes_that_share_a_glyph_in_one_lookup() {
    let src = "markClass acutecomb <anchor 0 500> @TOP;\nmarkClass acutecomb <anchor 0 -10> @BOTTOM;\nfeature mark {\n    pos base a <anchor 250 500> mark @TOP;\n    pos base a <anchor 250 0> mark @BOTTOM;\n} mark;\n";
    let diags = lint(src, &plain());
    assert_eq!(rules(&diags), [Rule::MarkClassConflict]);
    assert!(
        diags[0]
            .message
            .starts_with("acutecomb is in both @TOP and @BOTTOM")
    );
    // in separate lookups, one glyph may be in both
    let split = "markClass acutecomb <anchor 0 500> @TOP;\nmarkClass acutecomb <anchor 0 -10> @BOTTOM;\nfeature mark {\n    lookup T { pos base a <anchor 250 500> mark @TOP; } T;\n    lookup B { pos base a <anchor 250 0> mark @BOTTOM; } B;\n} mark;\n";
    assert!(codes(split).is_empty());
}

#[test]
fn reports_empty_features_and_lookups() {
    assert_eq!(
        codes("lookup E { } E;\nfeature liga { lookup E; } liga;\n"),
        ["L003"]
    );
    assert_eq!(
        codes("feature ss01 { featureNames { name \"Alt\"; }; } ss01;\n"),
        ["L003"]
    );
    assert!(
        codes("feature aalt { feature smcp; } aalt;\nfeature smcp { sub a by a.sc; } smcp;\n")
            .is_empty()
    );
}

#[test]
fn reports_a_languagesystem_after_a_feature_and_moves_it_up() {
    let src = "languagesystem DFLT dflt;\nfeature liga { sub f i by f_i; } liga;\nlanguagesystem latn dflt;\nfeature kern { pos a v -10; } kern;\n";
    let diags = lint(src, &plain());
    assert_eq!(rules(&diags), [Rule::LanguagesystemAfterFeature]);
    assert_eq!(diags[0].fixes[0].applicability, Applicability::Unsafe);
    let fixed = feanor::apply_fixes(src, &diags, Applicability::Unsafe);
    assert_eq!(
        fixed,
        "languagesystem DFLT dflt;\nlanguagesystem latn dflt;\nfeature liga { sub f i by f_i; } liga;\nfeature kern { pos a v -10; } kern;\n"
    );
}

#[test]
fn reports_stray_semicolons_with_fea_rs_severity_and_removes_them() {
    let src = "feature liga {\n    sub f i by f_i;;\n} liga;;\n";
    let diags = lint(src, &plain());
    assert_eq!(rules(&diags), [Rule::StraySemicolon, Rule::StraySemicolon]);
    assert_eq!(diags[0].severity, Severity::Warning, "inside a block");
    assert_eq!(diags[1].severity, Severity::Error, "at the top level");
    let fixed = feanor::apply_fixes(src, &diags, Applicability::Safe);
    assert_eq!(fixed, "feature liga {\n    sub f i by f_i;\n} liga;\n");
}

#[test]
fn a_repeat_with_the_same_result_is_redundant_not_a_conflict() {
    // Rokkitt, Coiny and Recursive repeat rules exactly; that changes nothing
    let ligature = "feature liga {\n    sub f f by f_f;\n    sub f f by f_f;\n} liga;\n";
    assert_eq!(codes(ligature), ["R006"]);
    let pair = "feature kern {\n    pos T o -60;\n    pos T o -60;\n} kern;\n";
    let diags = lint(pair, &plain());
    assert_eq!(rules(&diags), [Rule::RedundantRule]);
    assert_eq!(diags[0].severity, Severity::Hint);
    let fixed = feanor::apply_fixes(pair, &diags, Applicability::Safe);
    assert_eq!(fixed, "feature kern {\n    pos T o -60;\n} kern;\n");
    let ignore =
        "feature calt {\n    ignore sub x a';\n    ignore sub x a';\n    sub a' by b;\n} calt;\n";
    assert_eq!(codes(ignore), ["R006"]);
}

#[test]
fn a_longer_inline_ligature_after_a_shorter_one_still_forms() {
    // checked with HarfBuzz: both compilers put the ligatures in one lookup
    // that forms the longest match, so "x a b c" becomes "x abc"
    let src = "feature calt {\n    sub x a' b' by ab;\n    sub x a' b' c' by abc;\n} calt;\n";
    assert!(codes(src).is_empty());
    // an ignore still stops it
    let src = "feature calt {\n    ignore sub x a' b';\n    sub x a' b' c' by abc;\n} calt;\n";
    assert_eq!(codes(src), ["R001"]);
}

#[test]
fn a_class_means_its_definition_where_it_is_used() {
    // Noto: each lookup redefines @input1; both compilers use the definition
    // before the rule, so ALT applies to virama here, and isn't "doing nothing"
    let src = "lookup ALT { sub virama by virama.alt; } ALT;\nlookup ONE { @input1 = [ka]; sub @input1 by ka.alt; } ONE;\nlookup TWO { @input1 = [virama]; sub x @input1' lookup ALT; } TWO;\nfeature akhn { lookup ONE; lookup TWO; } akhn;\n";
    assert!(codes(src).is_empty(), "{:?}", codes(src));
    // and C001 compares the classes as defined where the rule is
    let src = "@A = [a b c];\n@B = [x y];\nfeature smcp { sub @A by @B; } smcp;\n@A = [a b];\n";
    assert_eq!(codes(src), ["C001"]);
}

#[test]
fn a_narrower_rule_that_does_the_same_is_redundant() {
    // Cambay: a longer backtrack with the same replacement never runs, and loses nothing
    let src = "feature calt {\n    sub b c a' by a.alt;\n    sub x b c a' by a.alt;\n} calt;\n";
    assert_eq!(codes(src), ["R006"]);
}

#[test]
fn spacing_doesnt_make_a_kerning_value_different() {
    let src = "feature kern {\n    pos A V <-80 0 -80 0>;\n    pos A V < -80 0 -80 0 >;\n} kern;\n";
    assert_eq!(codes(src), ["R006"]);
    // Arimo: a placement and an advance are different values, so the second is ignored
    let src = "feature kern {\n    pos A V <-41 0 0 0>;\n    pos A V -41;\n} kern;\n";
    assert_eq!(codes(src), ["R003"]);
}

#[test]
fn language_statements_split_a_lookup_block() {
    // Ubuntu, Lilex: the same substitution for each language, inside one lookup block
    let src = "feature locl {\n    lookup L {\n        script latn;\n        language ROM;\n        sub Scedilla by Scommaaccent;\n        language MOL;\n        sub Scedilla by Scommaaccent;\n    } L;\n} locl;\n";
    assert!(codes(src).is_empty());
}

#[test]
fn a_block_with_an_automatic_code_comment_isnt_empty() {
    // Amiri: ufo2ft inserts the generated mark rules at this comment
    assert!(codes("feature mark {\n# Automatic Code\n} mark;\n").is_empty());
}

#[test]
fn finds_languagesystem_order_in_files_with_old_mac_line_endings() {
    // Bungee: carriage returns only, so the whole file is one line to `\n`
    let src = "languagesystem DFLT dflt;\rlanguagesystem latn dflt;\rfeature liga { sub f i by f_i; } liga;\r";
    assert!(codes(src).is_empty());
    let late = "languagesystem DFLT dflt;\rfeature liga { sub f i by f_i; } liga;\rlanguagesystem latn dflt;\r";
    assert_eq!(codes(late), ["E003"]);
}

#[test]
fn judges_only_lookups_that_see_the_glyphs_the_rule_names() {
    // Kanit: once the ligature forms at l, the call at periodcentered sees a different sequence
    let src = "lookup LIG { sub l periodcentered l by ldot_l; } LIG;\nfeature locl {\n    sub l' lookup LIG periodcentered' lookup LIG l;\n} locl;\n";
    assert!(codes(src).is_empty());
    // a second lookup at one position sees the first one's result
    let src = "lookup A { sub e by e.alt; } A;\nlookup B { sub e.alt by e.alt2; } B;\nfeature calt {\n    sub e' lookup A lookup B t;\n} calt;\n";
    assert!(codes(src).is_empty());
}

#[test]
fn a_lone_semicolon_in_a_table_is_stray() {
    // Moderustic: fea-rs rejects it with "Expected Attach, ... found ;"; fontTools accepts it
    let src = "table GDEF {\n    GlyphClassDef [a], , , ;\n;\n} GDEF;\n";
    let diags = lint(src, &plain());
    assert_eq!(rules(&diags), [Rule::StraySemicolon]);
    // a missing operand isn't a stray semicolon
    assert_eq!(codes("feature liga { sub f i by ; } liga;\n"), ["E001"]);
}

#[test]
fn an_error_inside_a_multibyte_character_doesnt_crash() {
    // Galada with a planted no-break space: fea-rs points into the middle of it
    let src = "markClass acutecomb <anchor 0 0> @M;\nfeature mark {\n    pos base [a]\u{a0}<anchor 1 2> mark @M;\n} mark;\n";
    assert!(rules(&lint(src, &plain())).contains(&Rule::InvisibleCharacter));
}

#[test]
fn a_bare_include_is_an_error_not_a_crash() {
    // fea-rs panics on an include with no path, as when one is half typed
    for src in [
        "include",
        "include path",
        "feature liga { include } liga;\n",
        "@a = [x include y];\n",
    ] {
        let diags = lint(src, &plain());
        assert!(
            diags
                .iter()
                .any(|d| d.message.starts_with("include needs a file")),
            "{src:?}: {diags:?}"
        );
        assert_eq!(format(src, &four_spaces()).text, src, "left as written");
    }
    // in a comment or a string it's just a word
    assert!(lint("# include this\n", &plain()).is_empty());
    assert!(
        codes("include(a.fea);\ninclude (b.fea);\n")
            .iter()
            .all(|c| *c == "P001")
    );
}

#[test]
fn an_include_in_one_file_is_a_hint_to_lint_the_project() {
    let diags = lint(
        "include(classes.fea);\nfeature liga { sub f i by f_i; } liga;\n",
        &plain(),
    );
    assert_eq!(rules(&diags), [Rule::Project]);
    assert_eq!(diags[0].severity, Severity::Hint);
}

#[test]
fn suggests_glyph_names_only_for_likely_typos() {
    let font = glyphs(&["agrave", "b.sc", "u1F10", "uni00A0", "f_j", "a.ss02"]);
    let suggestion = |name: &str| {
        let src = format!("feature calt {{ sub {name} by b.sc; }} calt;\n");
        let diags = lint(&src, &LintOptions::default().with_glyphs(&font));
        let missing = diags
            .iter()
            .find(|d| d.rule == Rule::GlyphNotInFont)
            .unwrap();
        missing.fixes.first().map(|f| f.edits[0].new_text.clone())
    };
    assert_eq!(suggestion("agrve").as_deref(), Some("agrave"));
    assert_eq!(suggestion("bsc").as_deref(), Some("b.sc"));
    // a different code point, letter or numbered variant names a different glyph
    assert_eq!(suggestion("uFF10"), None);
    assert_eq!(suggestion("uni0000"), None);
    assert_eq!(suggestion("f_f"), None);
    assert_eq!(suggestion("a.ss01"), None);
}

// ------------------------------------------------------------------ projects

#[test]
fn projects_follow_includes_and_ifdefs_and_map_back_to_files() {
    let dir = std::env::temp_dir().join(format!("feanor-test-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("features")).unwrap();
    std::fs::write(
        dir.join("main.fea"),
        "include(features/classes.fea);\n#ifdef ITALIC\n#include \"features/italic.fea\"\n#endif\nfeature liga { sub @lc by f_i; } liga;\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("features/classes.fea"),
        "@lc = [f i];\n@italic_only = [a];\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("features/italic.fea"),
        "feature ss01 { sub @italic_only by b; } ss01;\n",
    )
    .unwrap();

    let roman = Project::from_root(&dir.join("main.fea"), &[]).unwrap();
    assert_eq!(roman.files().len(), 2);
    let found = roman.lint(&plain());
    let unused = found
        .iter()
        .find(|f| f.diagnostic.rule == Rule::UnusedClass)
        .expect("@italic_only unused without ITALIC");
    assert!(roman.files()[unused.file].ends_with(Path::new("features/classes.fea")));
    assert_eq!(
        &roman.text(unused.file)[unused.diagnostic.range.clone()],
        "@italic_only"
    );

    let italic = Project::from_root(&dir.join("main.fea"), &["ITALIC".into()]).unwrap();
    assert_eq!(italic.files().len(), 3);
    assert!(
        !italic
            .lint(&plain())
            .iter()
            .any(|f| f.diagnostic.rule == Rule::UnusedClass)
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_missing_include_is_reported_where_it_is_written() {
    let dir = std::env::temp_dir().join(format!("feanor-missing-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("main.fea"),
        "languagesystem DFLT dflt;\ninclude(nope.fea);\n",
    )
    .unwrap();
    let project = Project::from_root(&dir.join("main.fea"), &[]).unwrap();
    let found = project.lint(&plain());
    let problem = found
        .iter()
        .find(|f| f.diagnostic.rule == Rule::Project)
        .expect("missing include");
    assert_eq!(
        &project.text(problem.file)[problem.diagnostic.range.clone()],
        "include(nope.fea);"
    );
    std::fs::remove_dir_all(dir).ok();
}

// ------------------------------------------------------------------ regressions found on Google Fonts

#[test]
fn glyphs_number_expressions_keep_their_braces_tight() {
    // Playfair: a space after `${` changed how the expression is read
    let src = "feature mark {\n    pos base a <anchor ${x.afrc:width*4.5} 0> mark @M;\n} mark;\n";
    assert_eq!(fmt(src), src);
    assert_safe(src);
}

#[test]
fn a_blank_line_after_the_bracket_formats_the_same_twice() {
    // Farsan: the blank line forced a block, wasn't printed, and the next pass
    // undid the block. Now any break after `[` keeps the rows, and the blank
    // line itself (which would sit right after `[`) is dropped.
    assert_safe("@a = [\n\n    b c\n];\n");
    assert_eq!(fmt("@a = [\n\n    b c\n];\n"), "@a = [\n    b c\n];\n");
}

#[test]
fn a_file_that_does_not_parse_formats_the_same_twice() {
    // Jomhuria: `//` comments aren't FEA; recovery left an empty broken node at
    // the end, which gained a blank line on every pass
    let src =
        "@Digits = [ zero one];\n\n// used only for kerning\n@RaaWaw = [ @aRaa.fina @aWaw.isol];\n";
    assert_safe(src);
    let tail = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/broken-tail.fea"
    ))
    .unwrap();
    assert_safe(&tail);
}

#[test]
fn keeps_windows_line_endings() {
    let src = "feature liga {\r\nsub f i by f_i; # note\r\n} liga;\r\n";
    let out = fmt(src);
    assert_eq!(
        out,
        "feature liga {\r\n    sub f i by f_i; # note\r\n} liga;\r\n"
    );
    assert_safe(src);
}

#[test]
fn untouched_ranges_point_into_windows_files() {
    let src = "@a = [b];\r\n@broken = [c\r\nfeature x {} x;\r\n";
    let out = format(src, &four_spaces());
    assert!(!out.untouched.is_empty());
    for r in &out.untouched {
        assert!(
            src[r.clone()].starts_with("@broken"),
            "{:?}",
            &src[r.clone()]
        );
    }
}

#[test]
fn the_readme_lists_every_rule() {
    // the workspace README isn't packaged with the crate; check it when it's there
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md");
    let Ok(readme) = std::fs::read_to_string(path) else {
        return;
    };
    for rule in Rule::ALL {
        let row = format!("| {} | {} |", rule.code(), rule.name());
        assert!(
            readme.contains(&row),
            "README rule table is missing `{row}`"
        );
    }
}
