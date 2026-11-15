// Copy the WebAssembly build of Feanor (crates/feanor-wasm/pkg) into the site,
// rebuilding it first with crates/feanor-wasm/build.sh when it is missing or
// older than the Rust sources.
import { execFileSync, execSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = new URL("../../", import.meta.url);
const pkg = new URL("crates/feanor-wasm/pkg/", root);
const built = new URL("feanor_bg.wasm", pkg);

function newestMtime(url) {
  if (!existsSync(url)) return 0;
  const stat = statSync(url);
  if (!stat.isDirectory()) return stat.mtimeMs;
  let newest = 0;
  for (const entry of readdirSync(url)) {
    const child = new URL(entry + (statSync(new URL(entry, url)).isDirectory() ? "/" : ""), url);
    newest = Math.max(newest, newestMtime(child));
  }
  return newest;
}

function onPath(command) {
  try {
    execSync(`command -v ${command}`, { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

const sources = [
  "Cargo.toml",
  "Cargo.lock",
  "crates/feanor/Cargo.toml",
  "crates/feanor/src/",
  "crates/feanor-wasm/Cargo.toml",
  "crates/feanor-wasm/src/",
  "crates/feanor-wasm/build.sh",
].map((path) => new URL(path, root));
const stale =
  !existsSync(built) ||
  Math.max(...sources.map(newestMtime)) > statSync(built).mtimeMs;

if (stale) {
  const script = fileURLToPath(new URL("crates/feanor-wasm/build.sh", root));
  let command = [script];
  if (!onPath("wasm-bindgen") || !onPath("wasm-opt")) {
    if (!onPath("nix")) {
      console.error(
        "The WebAssembly build is missing or out of date, and building it needs " +
          "wasm-bindgen-cli 0.2.127 and wasm-opt (binaryen), or Nix to provide them.",
      );
      process.exit(1);
    }
    command = ["nix", "shell", "nixpkgs#wasm-bindgen-cli", "nixpkgs#binaryen", "--command", script];
  }
  console.log("Building the Feanor WebAssembly package...");
  try {
    execFileSync(command[0], command.slice(1), { stdio: "inherit" });
  } catch {
    process.exit(1);
  }
}

const wasmDir = new URL("../src/wasm/", import.meta.url);
mkdirSync(wasmDir, { recursive: true });
for (const file of ["feanor.js", "feanor.d.ts", "feanor_bg.wasm"]) {
  copyFileSync(new URL(file, pkg), new URL(file, wasmDir));
}
// served as a static file so the browser can stream-compile it
const publicDir = new URL("../public/", import.meta.url);
mkdirSync(publicDir, { recursive: true });
copyFileSync(built, new URL("feanor_bg.wasm", publicDir));
console.log("Copied the Feanor WebAssembly build into the site.");
