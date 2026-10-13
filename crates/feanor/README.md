# feanor

A formatter and linter for OpenType feature files (`.fea`), built on
[fea-rs](https://docs.rs/fea-rs)'s lossless syntax tree. For the `fea`
command, see [`feanor-cli`](https://crates.io/crates/feanor-cli).

```rust
use feanor::{format, lint, FormatOptions, GlyphNames, LintOptions, Rule};

let source = "feature smcp {\n    sub a by a.sc;\n    sub b by bsc;\n} smcp;\n";

// layout only: tokens and comments never change
let formatted = format(source, &FormatOptions::default());
assert_eq!(formatted.text, source);

// rules with stable codes, fixes marked safe or unsafe
let glyphs: GlyphNames = ["a", "b", "a.sc", "b.sc"].into_iter().collect();
let diagnostics = lint(source, &LintOptions::default().with_glyphs(&glyphs));
assert_eq!(diagnostics[0].rule, Rule::GlyphNotInFont);
assert_eq!(diagnostics[0].message, "glyph 'bsc' is not in the font; did you mean 'b.sc'?");
```

- [`format`] and [`lint`] take a whole feature file or a feature body (the
  inside of a `feature` block, as Glyphs stores it).
- [`Project`] lints many files as one document: FEA `include()`, preprocessor
  `#include` / `#ifdef`, Glyphs-style sections.
- [`Incremental`] keeps a document linted as it's edited.
- Ranges are byte offsets; [`LineIndex`] converts them to the line and UTF-16
  column that editors and the Language Server Protocol use.

Each rule's documentation is in [`rules/`](rules) and available at runtime
from `Rule::explanation`.

Licensed under either of Apache-2.0 or MIT, at your option.

[`format`]: https://docs.rs/feanor/latest/feanor/fn.format.html
[`lint`]: https://docs.rs/feanor/latest/feanor/fn.lint.html
[`Project`]: https://docs.rs/feanor/latest/feanor/struct.Project.html
[`Incremental`]: https://docs.rs/feanor/latest/feanor/struct.Incremental.html
[`LineIndex`]: https://docs.rs/feanor/latest/feanor/struct.LineIndex.html
