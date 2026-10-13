//! Behaviour tests: what the formatter prints, what the linter reports, and
//! that the incremental linter always agrees with a full lint.

use std::path::Path;

use feanor::internal::{first_token_mismatch, lint_whole_document};
use feanor::{
    Applicability, Diagnostic, FormatOptions, GlyphNames, Incremental, Indent, LintOptions,
    Project, Rule, apply_noqa, detect_indent, format, lint,
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
        "feature calt {{\n    sub {}' x by y;\n}} calt;\n",
        "glyphname ".repeat(10).trim_end()
    );
    let out = fmt(&src);
    assert!(out.contains("\n        by y;"), "{out}");
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
    let mut doc = Incremental::new(DEMO.repeat(3), plain());
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
fn a_blank_line_after_the_bracket_does_not_force_a_block() {
    // Farsan: the blank line forced a block, wasn't printed, and the next pass undid the block
    assert_safe("@a = [\n\n    b c\n];\n");
    assert_eq!(fmt("@a = [\n\n    b c\n];\n"), "@a = [b c];\n");
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
