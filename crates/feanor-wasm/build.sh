#!/usr/bin/env bash
# Build the npm package in crates/feanor-wasm/pkg.
#
# Needs the wasm32-unknown-unknown target, wasm-bindgen-cli at the exact
# version of the wasm-bindgen crate in Cargo.toml, and wasm-opt (binaryen).
# With Nix: nix shell nixpkgs#wasm-bindgen-cli nixpkgs#binaryen --command ./build.sh
set -euo pipefail
cd "$(dirname "$0")"
root=../..
out=pkg

cargo build --release --target wasm32-unknown-unknown -p feanor-wasm
rm -rf "$out"
wasm-bindgen --target web --out-dir "$out" --out-name feanor \
  "$root/target/wasm32-unknown-unknown/release/feanor_wasm.wasm"
wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int \
  "$out/feanor_bg.wasm" -o "$out/feanor_bg.wasm"

version=$(cargo metadata --format-version 1 --no-deps --manifest-path Cargo.toml \
  | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "feanor-wasm"))')
cat > "$out/package.json" <<JSON
{
  "name": "@shift-editor/feanor",
  "version": "$version",
  "description": "Formatter and linter for OpenType feature files (.fea), compiled to WebAssembly",
  "license": "MIT OR Apache-2.0",
  "repository": { "type": "git", "url": "https://github.com/shift-editor/feanor", "directory": "crates/feanor-wasm" },
  "keywords": ["opentype", "fonts", "fea", "linter", "formatter", "wasm"],
  "type": "module",
  "main": "feanor.js",
  "types": "feanor.d.ts",
  "files": ["feanor.js", "feanor.d.ts", "feanor_bg.wasm", "feanor_bg.wasm.d.ts", "README.md"],
  "sideEffects": false
}
JSON
cp README.md "$out/README.md"
ls -l "$out"/feanor_bg.wasm
printf 'gzipped: %s bytes\n' "$(gzip -9 -c "$out/feanor_bg.wasm" | wc -c)"
