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
}

/** A before/after diff of a feature file, rendered with @pierre/diffs. */
export function DiffView({ name, before, after, layout = "split" }: Props) {
  registerFea();
  if (before === after) {
    return (
      <p className="rounded-lg border border-line bg-surface px-4 py-6 text-center font-ui text-sm text-muted">
        Already formatted: nothing to change.
      </p>
    );
  }
  return (
    <div className="max-h-[36rem] overflow-auto rounded-lg border border-line">
      <MultiFileDiff
        oldFile={{ name, contents: before, lang: "fea" }}
        newFile={{ name, contents: after, lang: "fea" }}
        options={{
          diffStyle: layout,
          theme: { light: "pierre-light", dark: "pierre-light" },
          themeType: "light",
          overflow: "scroll",
          lineDiffType: "word",
        }}
      />
    </div>
  );
}
