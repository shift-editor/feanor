# Feanor

A fast formatter and linter for OpenType feature files (`.fea`), written in Rust.

Feanor is built on [fea-rs](https://github.com/googlefonts/fontc/tree/main/fea-rs),
the feature compiler used by Google's [fontc](https://github.com/googlefonts/fontc).
It can be used as a command-line tool, in CI, or as a library inside font editors
and language servers.

- **Safe formatting.** Only whitespace changes. Every token and comment is kept,
  in order, so formatting never changes the compiled font.
- **Useful diagnostics.** Each rule has a stable code, a severity, and
  documentation. Many come with automatic fixes.
- **Whole projects.** Follows `include()` statements, C-preprocessor
  `#include` / `#ifdef` blocks, and Glyphs-style feature sections.
- **Fast.** Formats or lints a 65,000-line feature file in about 0.3 seconds,
  and re-lints only what changed while you type.

## Installation

```console
cargo install feanor-cli
```

This installs the `feanor` binary. Rust 1.89 or later is required. To use
Feanor as a library, add the [`feanor`](https://crates.io/crates/feanor) crate.

## Usage

```console
feanor fmt features.fea               # print the formatted file
feanor fmt --write features.fea       # format in place
feanor fmt --check sources/*.fea      # exit with status 1 if any file would change

feanor lint features.fea --glyphs MyFont-Regular.ufo
feanor lint features.fea --fix        # apply safe fixes
feanor lint features.fea --statistics # count diagnostics by rule

feanor project sources/features.fea --all-builds   # lint a project and every #ifdef build
feanor check sources/features.fea                  # formatting and lint, for CI

feanor rule G001                      # explain a rule
```

Exit status is 0 when nothing is found, 1 when there are lint errors or
unformatted files, and 2 when a file can't be read.

### Continuous integration

```yaml
- run: cargo install feanor-cli
- run: feanor check sources/features.fea --format github
```

With `--format github`, diagnostics appear as annotations on pull requests.

## Formatting

The formatter changes spacing, indentation, line breaks and blank lines, and
nothing else.

- A glyph class written on one line is packed to the line width. A class that
  starts on a new line after `[` keeps your line breaks, and only lines that
  are too long are wrapped.
- Columns that were aligned by hand stay aligned, with the column widths recalculated.
- The file's existing indentation and line endings are kept.
- Code that doesn't parse is left exactly as written.
- `# fmt: off` and `# fmt: on` turn formatting off for a region.

Formatting is idempotent: running it again changes nothing.

## Rules

| Code | Rule | Severity | Fix |
|---|---|---|---|
| E001 | syntax | error | |
| E002 | stray-semicolon | warning or error | safe |
| E003 | languagesystem-after-feature | warning | unsafe |
| G001 | glyph-not-in-font | error | unsafe |
| C001 | class-length-mismatch | error | |
| C002 | duplicate-glyph-in-class | warning | safe |
| C003 | undefined-class | error | |
| C004 | unused-class | hint | |
| C005 | mark-class-conflict | error | |
| L001 | undefined-lookup | error | |
| L002 | unused-lookup | hint | |
| L003 | empty-block | warning | |
| R001 | unreachable-rule | warning | |
| R002 | duplicate-substitution | error | |
| R003 | duplicate-pair | warning | safe |
| R004 | wrong-lookup-type | error | |
| R005 | lookup-has-no-effect | warning | |
| R006 | redundant-rule | hint | safe |
| T001 | unknown-feature-tag | warning | unsafe |
| X001 | invisible-character | error | safe |
| P001 | project | error | |

Safe fixes never change what the font does, so `--fix` applies them
automatically. Unsafe fixes change the font, such as renaming a glyph or a
feature to a suggested name, and are only applied with `--unsafe-fixes`.

You can choose which rules run by code (`G001`), family (`C`), or name, using
`--select` and `--ignore`, the configuration file, or a `# noqa` comment on
the line. Each rule's documentation is in [`crates/feanor/rules`](crates/feanor/rules).

## Configuration

Feanor uses the nearest `feanor.toml` in the file's directory or any parent:

```toml
line-width = 100
indent = "auto"          # keep the file's indent; or a number of spaces, or "tab"
keep-alignment = true

[lint]
select = ["ALL"]
ignore = ["C004"]
glyphs = "sources/MyFont-Regular.ufo"   # a UFO, or a text file with one glyph name per line
defines = ["ITALIC"]                    # names defined for #ifdef

[lint.per-file-ignores]
"classes.fea" = ["unused-class"]
```

## Library

The [`feanor`](crates/feanor) crate exposes the formatter and linter:

```rust
use feanor::{format, lint, FormatOptions, GlyphNames, LintOptions, Rule};

let source = "feature smcp {\n    sub a by a.sc;\n    sub b by bsc;\n} smcp;\n";

let formatted = format(source, &FormatOptions::default());

let glyphs: GlyphNames = ["a", "b", "a.sc", "b.sc"].into_iter().collect();
let diagnostics = lint(source, &LintOptions::default().with_glyphs(&glyphs));
assert_eq!(diagnostics[0].rule, Rule::GlyphNotInFont);
```

Diagnostics use byte ranges. `LineIndex` converts them to the line and UTF-16
column positions that editors and the Language Server Protocol expect.
`Incremental` keeps a document linted as it's edited.

## Testing

Besides unit tests, Feanor is checked against the feature code of every Google
Fonts family (about 2,700 files):

- Formatting a file twice gives the same result.
- No token or comment changes.
- Files that compile on their own produce identical layout tables before and
  after formatting, using both fea-rs and fontTools feaLib.

The linter is checked by adding known mistakes to working files and confirming
that each one is reported at the right place. See [CONTRIBUTING.md](CONTRIBUTING.md)
to run these checks.

## Crates

| Crate | Description |
|---|---|
| [`feanor`](crates/feanor) | Library: formatting, linting, projects, incremental linting, configuration |
| [`feanor-cli`](crates/feanor-cli) | The `feanor` command-line tool |
| [`feanor-wasm`](crates/feanor-wasm) | WebAssembly build for browsers and Node |

## License

Licensed under either the [Apache License, Version 2.0](LICENSE-APACHE) or the
[MIT license](LICENSE-MIT), at your option.
