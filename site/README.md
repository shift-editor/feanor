# feanor.shift.graphics

The landing page for Feanor, with a live playground running Feanor's
WebAssembly build. Next 16, Tailwind 4, CodeMirror 6 and @pierre/diffs.

```console
nix shell nixpkgs#wasm-bindgen-cli nixpkgs#binaryen --command ../crates/feanor-wasm/build.sh
pnpm install
pnpm dev            # copies the WebAssembly build in, then starts Next
```

`pnpm build` does the same for production. `node scripts/smoke.mjs URL` loads
a running site in headless Chromium and checks every interactive part.

- `examples/`: the feature files shown on the page, with credits in `credits.json`
- Rule documentation is read from `../crates/feanor/rules` at build time, so the
  page always matches the library.

## Deploying

The page is static. The build needs the WebAssembly package first, so a
deployment must build `crates/feanor-wasm` (Rust with the
`wasm32-unknown-unknown` target, wasm-bindgen-cli 0.2.127, binaryen) before
`pnpm build`.
