// Copy the WebAssembly build of Feanor (crates/feanor-wasm/pkg) into the site.
// Build it first: crates/feanor-wasm/build.sh
import { copyFileSync, existsSync, mkdirSync } from "node:fs";

const pkg = new URL("../../crates/feanor-wasm/pkg/", import.meta.url);
if (!existsSync(new URL("feanor_bg.wasm", pkg))) {
  console.error("No WebAssembly build found. Run crates/feanor-wasm/build.sh first.");
  process.exit(1);
}
const wasmDir = new URL("../src/wasm/", import.meta.url);
mkdirSync(wasmDir, { recursive: true });
for (const file of ["feanor.js", "feanor.d.ts", "feanor_bg.wasm"]) {
  copyFileSync(new URL(file, pkg), new URL(file, wasmDir));
}
// served as a static file so the browser can stream-compile it
copyFileSync(new URL("feanor_bg.wasm", pkg), new URL("../public/feanor_bg.wasm", import.meta.url));
console.log("Copied the Feanor WebAssembly build into the site.");
