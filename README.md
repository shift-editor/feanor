# feanor

A formatter and linter for OpenType feature files (`.fea`), built on
[fea-rs](https://github.com/googlefonts/fontc/tree/main/fea-rs), the feature
compiler in Google's [fontc](https://github.com/googlefonts/fontc).

| Crate | What |
|---|---|
| [`feanor`](crates/feanor) | The library: `format`, `lint`, projects, incremental linting, `feanor.toml` |
| [`feanor-cli`](crates/feanor-cli) | The `feanor` command |

```console
$ feanor fmt --write features.fea           # format in place
$ feanor lint features.fea --glyphs Font.ufo
$ feanor check sources/features.fea --format github   # CI: formatting + lint, PR annotations
```

## What it does

- **Formats layout only.** Spacing, indentation, line breaks, blank lines.
  Every token and comment comes out with the same text in the same order, so
  the compiled font can't change. Glyph classes are packed to the line width;
  columns the author aligned stay aligned; `# fmt: off` leaves a region
  alone; the file's own indent and line endings are kept.
- **Lints** with rules that each have a short code, a severity and a page of
  documentation (`feanor rule G001`):

  | Code | Rule | Severity | Fix |
  |---|---|---|---|
  | E001 | syntax | error | |
  | G001 | glyph-not-in-font | error | unsafe |
  | C001 | class-length-mismatch | error | |
  | C002 | duplicate-glyph-in-class | warning | safe |
  | C003 | undefined-class | error | |
  | C004 | unused-class | hint | |
  | L001 | undefined-lookup | error | |
  | L002 | unused-lookup | hint | |
  | T001 | unknown-feature-tag | warning | unsafe |
  | X001 | invisible-character | error | safe |
  | P001 | project | error | |

  Rules can be selected and ignored by code, family or name, in `feanor.toml`,
  per file, or with `# noqa` comments, as in [Ruff](https://docs.astral.sh/ruff/).
- **Understands projects**: FEA `include()`, C-preprocessor `#include` /
  `#ifdef` (and lints every `#ifdef` build), and Glyphs-style sections.
- **Keeps up with typing**: the incremental linter re-parses only the
  top-level items an edit touches.

## How we know the output is right

The formatter's guarantee is that it never changes what the font does. That's
checked several independent ways, over the feature code of every Google Fonts
family's upstream repository (2,762 files, 2.7 million lines):

| Check | Result |
|---|---|
| Formatting twice gives the same text | 2,751 of 2,751 |
| The stream of tokens and comments is unchanged | 2,751 of 2,751 |
| fea-rs compiles the file to identical GSUB/GPOS/GDEF before and after | 1,901 of 1,901 that compile on their own |
| fontTools feaLib (an independent compiler) does the same | 1,923 of 1,923 that compile on their own |

(11 files aren't UTF-8 and are reported as such; the rest are fragments that
need other files to compile.) Truncating files at 200 points each, as when
someone is typing, produced no crashes and kept every guarantee.

The linter is checked by planting known defects in files that compile cleanly
and checking that the right rule fires at the right place: 2,749 of 2,749
planted defects found across 360 files, with no error-level diagnostics on the
unmodified files. For the defects that are compile errors, feaLib also rejects
the planted file, confirming they're real.

The scripts are in [`tools/oracle`](tools/oracle); see
[CONTRIBUTING.md](CONTRIBUTING.md) to run them.

## Status

Not yet published. Licensed under either of [Apache-2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT), at your option.
