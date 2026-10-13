"use client";

import { registerCustomLanguage } from "@pierre/diffs";
import { MultiFileDiff } from "@pierre/diffs/react";
import { feaGrammar } from "@/lib/fea-grammar";

// once per page load: teach the diff highlighter FEA, for .fea file names too
let registered = false;
function registerFea() {
  if (registered) return;
  registered = true;
  registerCustomLanguage("fea", async () => ({ default: [feaGrammar] }), ["fea"]);
}

interface Props {
  name: string;
  before: string;
  after: string;
  layout?: "split" | "unified";
  /** Height limit, as CSS; the diff scrolls past it. */
  maxHeight?: string;
  /**
   * A fixed height, as CSS. Long diffs need one: the diff renders only the
   * rows its scroll box shows, and a box sized by its content starts at zero.
   */
  height?: string;
}

/**
 * A before/after diff of a feature file, rendered with @pierre/diffs, without
 * a header or frame: it sits inside an AppWindow, which names the file.
 */
export function DiffView({ name, before, after, layout = "split", maxHeight = "36rem", height }: Props) {
  registerFea();
  if (before === after) {
    return <p className="px-4 py-6 text-center font-ui text-sm text-muted">Already formatted: nothing to change.</p>;
  }
  return (
    <div className="overflow-auto" style={{ maxHeight, height }}>
      <MultiFileDiff
        oldFile={{ name, contents: before, lang: "fea" }}
        newFile={{ name, contents: after, lang: "fea" }}
        options={{
          diffStyle: layout,
          theme: { light: "pierre-light", dark: "pierre-light" },
          themeType: "light",
          overflow: "scroll",
          lineDiffType: "word",
          // the AppWindow's title bar names the file; hidden rather than disabled,
          // since a diff without its header renders no rows (@pierre/diffs 1.5.1)
          unsafeCSS: "[data-diffs-header] { display: none; }",
        }}
      />
    </div>
  );
}
