"use client";

import { type ReactNode, useRef, useState } from "react";
import { FeaEditor, type FeaEditorHandle } from "./FeaEditor";
import type { Diagnostic } from "@/lib/feanor";

const BADGE = {
  error: "bg-danger-surface text-danger",
  warning: "bg-[#fdf3dc] text-[#8a5a00]",
  hint: "bg-surface-muted text-muted",
};

/** An editor with deliberate mistakes, the diagnostics Feanor finds, and their fixes. */
export function LintDemo({
  initial,
  glyphs,
  explanations,
}: {
  initial: string;
  glyphs: string[];
  /** Each rule's documentation, rendered on the server, by code. */
  explanations: Record<string, ReactNode>;
}) {
  const editor = useRef<FeaEditorHandle>(null);
  const [diagnostics, setDiagnostics] = useState<Diagnostic[]>([]);
  const [explained, setExplained] = useState<string | null>(null);

  function explain(code: string) {
    setExplained(explained === code ? null : code);
  }

  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_22rem]">
      <div className="overflow-hidden rounded-xl border border-line">
        <FeaEditor ref={editor} initial={initial} glyphs={glyphs} height="20rem" onDiagnostics={setDiagnostics} />
      </div>
      <div className="flex min-w-0 flex-col gap-2">
        <ul className="flex flex-col gap-2">
          {diagnostics.map((d) => (
            <li key={`${d.code}-${d.from}`} className="rounded-lg border border-line bg-surface-raised p-3 font-ui text-sm">
              <div className="flex items-baseline gap-2">
                <span className={`rounded px-1.5 py-0.5 font-mono text-[11px] ${BADGE[d.severity]}`}>{d.severity}</span>
                <button type="button" onClick={() => explain(d.code)} className="font-mono text-xs text-accent hover:underline">
                  {d.code} {d.rule}
                </button>
                <span className="ml-auto font-mono text-[11px] text-muted">line {d.start.line + 1}</span>
              </div>
              <p className="mt-1.5 text-primary">{d.message}</p>
              {d.fixes.length > 0 && (
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {d.fixes.map((fix) => (
                    <button
                      key={fix.title}
                      type="button"
                      onClick={() => editor.current?.applyEdits(fix.edits)}
                      className={fix.applicability === "safe" ? "button-primary" : "button-quiet"}
                    >
                      {fix.title}
                      {fix.applicability === "unsafe" && <span className="ml-1 text-muted">(unsafe)</span>}
                    </button>
                  ))}
                </div>
              )}
            </li>
          ))}
          {diagnostics.length === 0 && <li className="font-ui text-sm text-muted">No problems found.</li>}
        </ul>
        {explained && (
          <div className="rounded-lg border border-line bg-surface p-4">{explanations[explained]}</div>
        )}
      </div>
    </div>
  );
}
