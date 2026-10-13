# Contributing

## Checks

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## WebAssembly

`crates/feanor-wasm/build.sh` builds the package into `crates/feanor-wasm/pkg`.
It needs `wasm-bindgen-cli` at the version pinned in that crate's
`Cargo.toml`, and `wasm-opt`; with Nix:

```console
nix shell nixpkgs#wasm-bindgen-cli nixpkgs#binaryen --command crates/feanor-wasm/build.sh
node --test crates/feanor-wasm/test/smoke.mjs
```

## Correctness harnesses

The formatter promises never to change what a font does; the linter promises
to report real problems at the right place. Unit tests can't show that for
real-world feature code, so `tools/oracle` checks it against corpora:

- `feanor dev verify FILE [--glyphs LIST]`: formats twice, compares the token
  streams, and compiles before and after with fea-rs.
- `feanor dev truncate FILE`: formats the file cut at 200 points (half-typed code).
- `feanor dev incremental FILE`: applies hundreds of edits through the
  incremental linter and compares every result with a full lint.
- `tools/oracle/fealib_equiv.py`: compiles before and after formatting with
  fontTools feaLib, an implementation independent of fea-rs.
- `tools/oracle/lint_mutations.py`: plants known defects (a misspelled glyph,
  a repeated glyph, an invisible character, a bad tag, a short class, an
  undefined class) in clean files and checks each is found where it was planted.

Both Python scripts need fontTools (`uvx --from fonttools python …` works).

A formatter bug usually shows up as a file that isn't idempotent or whose
tokens change. When the harness finds one, add the smallest input that
reproduces it to `crates/feanor/tests/behaviour.rs` before fixing it.
