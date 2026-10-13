"use client";

import { type ReactNode, useRef, useState } from "react";
import { FeaEditor, type FeaEditorHandle } from "./FeaEditor";
import { AppWindow, ProblemCount } from "./Window";
import type { Diagnostic } from "@/lib/feanor";

// Shift's problem colours, as in the editor
const DOT = {
  error: "bg-[#dc2626]",
  warning: "bg-[#f97316]",
  hint: "bg-[#9f9f9f]",
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
  const [cursor, setCursor] = useState<[number, number]>([1, 1]);

  function explain(code: string) {
    setExplained(explained === code ? null : code);
  }

  return (
    <div className="grid items-start gap-6 lg:grid-cols-[minmax(0,1fr)_22rem]">
      <AppWindow
        title="Feature Code"
        detail="Demo Sans"
        status={
          <>
            <ProblemCount diagnostics={diagnostics} />
            <span className="flex-1" />
            <span className="tabular-nums">
              {cursor[0]}:{cursor[1]}
            </span>
            <span>FEA</span>
          </>
        }
      >
        <FeaEditor
          ref={editor}
          initial={initial}
          glyphs={glyphs}
          height="20rem"
          onDiagnostics={setDiagnostics}
          onCursor={(line, column) => setCursor([line, column])}
        />
      </AppWindow>
      <div className="flex min-w-0 flex-col gap-1 font-ui">
        <p className="px-3 py-1 text-[15px] font-medium" aria-live="polite">
          {diagnostics.length === 0 ? "No problems" : `${diagnostics.length} problem${diagnostics.length === 1 ? "" : "s"}`}
        </p>
        <ul>
          {diagnostics.map((d, i) => (
            <li
              key={`${d.code}-${d.from}`}
              className={`flex flex-col gap-2 rounded-md px-3 py-2.5 ${i % 2 === 0 ? "bg-stripe" : ""}`}
            >
              <div className="grid grid-cols-[0.5rem_minmax(0,1fr)_auto] items-baseline gap-x-2.5">
                <span aria-label={d.severity} className={`size-2 -translate-y-px rounded-full ${DOT[d.severity]}`} />
                <p className="text-[14px] leading-snug text-primary">{d.message}</p>
                <span className="font-mono text-[11px] text-muted">L{d.start.line + 1}</span>
              </div>
              <div className="flex flex-wrap items-center gap-1.5 pl-[1.125rem]">
                <button
                  type="button"
                  onClick={() => explain(d.code)}
                  aria-expanded={explained === d.code}
                  title={d.rule}
                  className="cursor-pointer rounded bg-chrome px-1.5 py-0.5 font-mono text-[11px] text-primary hover:bg-control-muted aria-expanded:bg-primary aria-expanded:text-white"
                >
                  {d.code}
                </button>
                {d.fixes.map((fix) => (
                  <button
                    key={fix.title}
                    type="button"
                    onClick={() => editor.current?.applyEdits(fix.edits)}
                    title={fix.applicability === "unsafe" ? "Unsafe: may change what the font does" : undefined}
                    className="cursor-pointer rounded border border-line-subtle bg-surface-raised px-1.5 py-0.5 text-[12px] text-primary transition-colors hover:bg-input"
                  >
                    {fix.title}
                    {fix.applicability === "unsafe" && <span className="text-muted"> · unsafe</span>}
                  </button>
                ))}
              </div>
            </li>
          ))}
        </ul>
        {explained && <div className="mt-3 px-3">{explanations[explained]}</div>}
      </div>
    </div>
  );
}
