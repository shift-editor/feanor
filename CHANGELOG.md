# Changelog

All notable changes are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). Rule codes and names are part of
the public interface: a rule is never renamed or renumbered within a major
version.

## Unreleased

### Added

- `FORCE_COLOR` and `CLICOLOR_FORCE` make the CLI print in colour when its
  output isn't a terminal (a browser terminal, a CI log that shows colours).
  `NO_COLOR` still turns colour off.

## 0.3.0 - 2026-10-14

### Added

- Rules about what lookups do, checked on the feature code of every Google
  Fonts family and against HarfBuzz where shaping decides:
  - `R001` unreachable-rule: a contextual rule an earlier rule in the same
    lookup always matches first, while doing something else
  - `R002` duplicate-substitution: a glyph or ligature substituted twice with
    different results (fea-rs keeps the last, fontTools rejects the file)
  - `R003` duplicate-pair: a glyph pair positioned twice with different values;
    the later one is ignored (safe fix: remove it)
  - `R004` wrong-lookup-type: a positioning lookup called from a substitution,
    or the reverse
  - `R005` lookup-has-no-effect: a lookup called on glyphs it has no rule for
  - `R006` redundant-rule: a rule that repeats what an earlier one does (hint;
    safe fix: remove it)
  - `C005` mark-class-conflict: mark classes sharing a glyph in one lookup
  - `L003` empty-block: a feature or lookup with no rules
  - `E002` stray-semicolon: a `;` with no statement (safe fix: remove it)
  - `E003` languagesystem-after-feature (unsafe fix: move it up)
- `feanor dev compile`: fea-rs's full errors and warnings for a file.

### Changed

- Linting one file reports each `include` as a `P001` hint to lint the whole
  project, instead of an `E001` error.
- Glyph name suggestions skip names that differ in a code point (`uFF10`,
  `u1F10`), a one-letter part (`f_f`, `f_j`) or only digits (`ss01`, `ss02`),
  which name different glyphs rather than typos.
- `format` lays out long rules differently (below), so code formatted by 0.2
  can change once when formatted again: about 1 file in 6 across the Google
  Fonts sources. Compiled output is unchanged.
- `format`: a glyph class that starts on a new line after `[` keeps the
  author's line breaks, so rows that group glyphs, or that line up with a
  parallel class, survive formatting. Only lines that are too long are
  wrapped. Classes written on one line are packed to the line width as before.
- `format`: when a long rule needs a class split over lines, `by` stays on the
  rule's line if it fits there.
- `format`: a contextual rule whose glyph sequence is too long for one line
  puts each class on its own line, instead of splitting short classes
  across lines; a run of single glyphs fills each line. A class is still
  split when it can't fit on a line of its own.

### Fixed

- Indent detection no longer picks an unusual unit from uneven nesting (a
  file indented 3 spaces, then 8, was formatted with 5); a tab or up to four
  spaces wins whenever the file uses one.
- A bare `include` with no file in parentheses is reported as an error
  instead of crashing (also in 0.1.0).
- `format`: rows that line up without needing any padding are formatted as
  ordinary statements, not as a table.
- `format` reports `# fmt: off` regions in `untouched`, as documented.
- `Incremental` (and the WebAssembly `Document`) report what `lint` reports
  for a feature body too, instead of syntax errors. A text is tried as a
  feature body only when it has syntax errors and no feature block that
  parses, since a body can't contain one.

## 0.2.0 - 2026-10-13

- `symbols`: where glyph classes, mark classes, and lookups are defined and used, and
  where glyph names are written; `Symbols::at` finds the symbol under a cursor and
  `Symbols::rename` returns the edits that rename a class or lookup everywhere
- `Incremental::symbols`: the same for a document being edited

## 0.1.0 - 2026-10-13

First release.

- `format`: layout-only formatter; keeps tokens, comments, the file's indent and line endings
- `lint`: rules E001, G001, C001–C004, L001, L002, T001, X001, P001
- Projects: FEA `include()`, preprocessor `#include` / `#ifdef`, Glyphs-style manifests
- Incremental linting for editors
- `feanor.toml` settings, `# noqa` suppressions, safe and unsafe fixes
- `feanor` command: `fmt`, `lint`, `project`, `check`, `rule`
