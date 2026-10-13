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
