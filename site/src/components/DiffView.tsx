"use client";

import { MultiFileDiff } from "@pierre/diffs/react";

interface Props {
  name: string;
  before: string;
  after: string;
  layout?: "split" | "unified";
}

/** A before/after diff of a feature file, rendered with @pierre/diffs. */
export function DiffView({ name, before, after, layout = "split" }: Props) {
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
        oldFile={{ name, contents: before, lang: "text" }}
        newFile={{ name, contents: after, lang: "text" }}
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
