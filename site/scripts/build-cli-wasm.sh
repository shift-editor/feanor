#!/bin/sh
# Build the feanor command-line tool for WASI, size-optimised, into
# public/feanor-cli.wasm, for the terminal on the page. Needs rustup's
# wasm32-wasip1 target and binaryen's wasm-opt (`nix shell nixpkgs#binaryen`).
set -eu
cd "$(dirname "$0")/../.."
env -u RUSTC CARGO_PROFILE_RELEASE_OPT_LEVEL=z CARGO_PROFILE_RELEASE_PANIC=abort \
  RUSTC="$(rustup which rustc)" "$(rustup which cargo)" build --release -p feanor-cli \
  --target wasm32-wasip1 --target-dir target/wasi
wasm-opt -Oz --strip-debug --strip-producers --enable-bulk-memory --enable-nontrapping-float-to-int \
  --enable-sign-ext --enable-mutable-globals \
  target/wasi/wasm32-wasip1/release/feanor.wasm -o site/public/feanor-cli.wasm
ls -l site/public/feanor-cli.wasm
