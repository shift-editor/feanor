// Loads Feanor's WebAssembly build once, on first use, in the browser.
import type * as Feanor from "@/wasm/feanor";

export type { Diagnostic, Formatted, FormatOptions, LintOptions, RuleInfo } from "@/wasm/feanor";
export type FeanorModule = typeof Feanor;

let loading: Promise<FeanorModule> | null = null;

export function loadFeanor(): Promise<FeanorModule> {
  loading ??= (async () => {
    const feanor = await import("@/wasm/feanor");
    await feanor.default({ module_or_path: "/feanor_bg.wasm" });
    return feanor;
  })();
  return loading;
}
