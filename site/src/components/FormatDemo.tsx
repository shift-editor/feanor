"use client";

import { useEffect, useMemo, useState } from "react";
import { Checkbox, Labelled, Segmented, Select, Slider } from "./controls";
import { DiffView } from "./DiffView";
import { AppWindow } from "./Window";

const lines = (text: string) => text.split("\n").length - (text.endsWith("\n") ? 1 : 0);
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
      <div className="flex flex-wrap items-end gap-x-5 gap-y-3">
        <div className="w-52">
          <Labelled label="File">
            <Select value={id} onChange={setId} options={examples.map((e) => ({ value: e.id, label: e.label }))} />
          </Labelled>
        </div>
        <div className="w-36">
          <Labelled label="Indent">
            <Select
              value={indent}
              onChange={setIndent}
              options={[
                { value: "auto", label: "Keep the file's" },
                { value: "2", label: "2 spaces" },
                { value: "4", label: "4 spaces" },
                { value: "tab", label: "Tab" },
              ]}
            />
          </Labelled>
        </div>
        <div className="w-40">
          <Labelled label={`Line width: ${width}`}>
            <Slider value={width} onChange={setWidth} min={60} max={140} step={5} label="Line width" />
          </Labelled>
        </div>
        <Checkbox checked={keepAlignment} onChange={setKeepAlignment}>
          Keep hand alignment
        </Checkbox>
        <div className="ml-auto">
          <Segmented
            label="Diff layout"
            value={layout}
            onChange={setLayout}
            options={[
              { value: "split", label: "Split" },
              { value: "unified", label: "Unified" },
            ]}
          />
        </div>
      </div>
      <AppWindow
        title="Format"
        detail={example.file}
        status={
          result && (
            <>
              <span>Formatted in {result.ms.toFixed(1)} ms, in your browser</span>
              <span className="flex-1" />
              <span className="tabular-nums">
                {lines(example.text)} → {lines(result.text)} lines
              </span>
              <span>FEA</span>
            </>
          )
        }
      >
        {result ? (
          <DiffView name={example.file} before={example.text} after={result.text} layout={layout} height="32rem" />
        ) : (
          <p className="px-4 py-6 font-ui text-sm text-muted">Loading Feanor…</p>
        )}
      </AppWindow>
      <p className="font-ui text-xs text-muted">{example.credit}</p>
    </div>
  );
}
