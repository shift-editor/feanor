# Possible fea-rs issues found while building feanor

Not reported upstream. Each has a minimal input here.

## 1. Numeric glyph ranges leave out their last glyph

`numeric-range.fea`:

```fea
sub [x.001 - x.003] by [a b c];
```

- fea-rs 1.0.0 (and fontc `main` as of 2026-10-13): fails with
  "class has different length (2) than target (3)". The range expands to
  `x.001 x.002`.
- fontTools feaLib: compiles; the range is `x.001 x.002 x.003`.
- The [feature file spec](https://adobe-type-tools.github.io/afdko/OpenTypeFeatureFileSpecification.html#2.g.ii)
  describes ranges as including both ends.
- Likely cause: `fea-rs/src/compile/glyph_range.rs`, `named()`, calls
  `num_range(&start.text, one..two, …)`; `one..two` excludes `two`.
  Letter ranges (`a - c`) use `start_char..=end_char` and are correct.

## 2. Contextual rules with hyphenated glyph names don't parse without a glyph map

`hyphenated-contextual.fea`, parsed with `fea_rs::parse::parse_string` (no
glyph map):

```fea
sub lam-ar.init' lookup LamAlef alef-ar.fina' lookup LamAlef;
```

reports "unexpected tokens in rewriter, please file a bug?". Without a glyph
map, `lam-ar.init` is a `GlyphNameOrRange` token, which the contextual-rule
rewriter doesn't expect. With a glyph map containing the names it parses.
The message itself asks for a report. feanor works around it by building a
parse-only glyph map from every hyphenated word in the text.

## 3. A bare `include` panics the parser

`bare-include.fea` is the single word:

```fea
include
```

`parse_root` (and `parse_string`) panic at `token_tree/typed.rs:555`
(`Include::path` unwraps a missing `Path` token) while resolving includes.
Anyone typing `include(...)` in an editor passes through this state. feanor
hides a bare `include` from the parser and reports it itself (`bare_includes`
in `crates/feanor/src/fmt.rs`).

## 4. The compiler panics on real sources at `compile/lookups/contextual.rs:379`

`contextual-panic.fea` is the smallest part of a Google Fonts family's source
that still panics (`called Option::unwrap() on a None value`): a contextual
rule calling a lookup at its second marked glyph, among empty lookup blocks.
Two more Google Fonts families' sources panic at the same line. fontTools
compiles all three.

## 5. The compiler panics on afdko's invalid-input tests

From `afdko/tests/addfeatures_data/input/spec/*.bad.fea`, files makeotf is
expected to reject: fea-rs panics instead of reporting an error at
`compile/compile_ctx.rs:1629`, `:1645`, `:1887` and `:2032`,
`compile/features.rs:521`, `compile/tables/stat.rs:64` and `:145`, and
`token_tree/typed.rs:710` (13 files). Reproduce with
`feanor dev compile FILE`.

## Other observations (probably by design; worth confirming)

- Token positions (`Token::range`, `Node::range`) are only filled in when a
  parse includes more than one file; single-file parses report 0. feanor
  tracks offsets itself.
- Includes are spliced into the tree in place of the `include` statement, so a
  single file with includes can't be reproduced from its tree. An option to
  parse one file and keep its `IncludeNode`s would let editors format a file
  on its own.
- Error recovery can add a varying number of empty `Eof` tokens, and can
  leave an empty error node at the end of the file.
