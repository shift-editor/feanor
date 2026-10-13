"use client";

import { useEffect, useMemo, useState } from "react";
import { DiffView } from "./DiffView";
import { type FeanorModule, loadFeanor } from "@/lib/feanor";

export interface Example {
  id: string;
  label: string;
  file: string;
  text: string;
  credit: string;
}

type IndentChoice = "auto" | "2" | "4" | "tab";

/** Real feature files from open-source fonts, formatted live with adjustable options. */
export function FormatDemo({ examples }: { examples: Example[] }) {
  const [feanor, setFeanor] = useState<FeanorModule | null>(null);
  const [id, setId] = useState(examples[0].id);
  const [indent, setIndent] = useState<IndentChoice>("auto");
  const [width, setWidth] = useState(100);
  const [keepAlignment, setKeepAlignment] = useState(true);
  const [layout, setLayout] = useState<"split" | "unified">("split");

  useEffect(() => {
    loadFeanor().then(setFeanor);
  }, []);

  const example = examples.find((e) => e.id === id) ?? examples[0];
  const result = useMemo(() => {
    if (!feanor) return null;
    const started = performance.now();
    const out = feanor.format(example.text, {
      indent: indent === "auto" || indent === "tab" ? indent : Number(indent),
      lineWidth: width,
      keepAlignment,
    });
    return { text: out.text, ms: performance.now() - started };
  }, [feanor, example, indent, width, keepAlignment]);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end gap-x-6 gap-y-3 font-ui text-sm">
        <label className="flex flex-col gap-1">
          <span className="text-xs text-muted">File</span>
          <select value={id} onChange={(e) => setId(e.target.value)} className="control">
            {examples.map((e) => (
              <option key={e.id} value={e.id}>
                {e.label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-muted">Indent</span>
          <select value={indent} onChange={(e) => setIndent(e.target.value as IndentChoice)} className="control">
            <option value="auto">Keep the file's</option>
            <option value="2">2 spaces</option>
            <option value="4">4 spaces</option>
            <option value="tab">Tab</option>
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-muted">Line width: {width}</span>
          <input type="range" min={60} max={140} step={5} value={width} onChange={(e) => setWidth(Number(e.target.value))} />
        </label>
        <label className="flex items-center gap-2 pb-1.5">
          <input type="checkbox" checked={keepAlignment} onChange={(e) => setKeepAlignment(e.target.checked)} />
          Keep hand alignment
        </label>
        <div className="flex gap-1 pb-0.5" role="group" aria-label="Diff layout">
          {(["split", "unified"] as const).map((l) => (
            <button key={l} type="button" aria-pressed={layout === l} onClick={() => setLayout(l)} className="segment">
              {l === "split" ? "Split" : "Unified"}
            </button>
          ))}
        </div>
      </div>
      {result ? (
        <DiffView name={example.file} before={example.text} after={result.text} layout={layout} />
      ) : (
        <p className="font-ui text-sm text-muted">Loading Feanor…</p>
      )}
      <p className="font-ui text-xs text-muted">
        {result && `Formatted in ${result.ms.toFixed(1)} ms in your browser. `}
        {example.credit}
      </p>
    </div>
  );
}
