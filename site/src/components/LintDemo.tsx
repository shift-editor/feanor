"use client";

import { CircleX, Info, TriangleAlert } from "lucide-react";
import { type ReactNode, useRef, useState } from "react";
import { FeaEditor, type FeaEditorHandle } from "./FeaEditor";
import { AppWindow, ProblemCount } from "./Window";
import type { Diagnostic } from "@/lib/feanor";

function SeverityIcon({ severity }: { severity: Diagnostic["severity"] }) {
  const className = "size-3.5 shrink-0";
  if (severity === "error") return <CircleX aria-label="Error" className={`${className} text-[#dc2626]`} />;
  if (severity === "warning") return <TriangleAlert aria-label="Warning" className={`${className} text-[#f97316]`} />;
  return <Info aria-label="Hint" className={`${className} text-[#9f9f9f]`} />;
}

/**
 * One file with many mistakes, laid out as Shift's feature code window: the
 * code, then its problems docked underneath. Choosing a problem selects it in
 * the code and opens its fixes and what the rule is about.
 */
export function LintDemo({
  initial,
  glyphs,
  explanations,
}: {
  initial: string;
  glyphs: string[];
  /** A line on each rule, rendered on the server, by code. */
  explanations: Record<string, ReactNode>;
}) {
  const editor = useRef<FeaEditorHandle>(null);
  const [diagnostics, setDiagnostics] = useState<Diagnostic[] | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const [cursor, setCursor] = useState<[number, number]>([1, 1]);
  const key = (d: Diagnostic) => `${d.code}-${d.from}`;

  function choose(d: Diagnostic) {
    setOpen(open === key(d) ? null : key(d));
    editor.current?.select(d.from, d.to);
  }

  return (
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
        height="22rem"
        onDiagnostics={setDiagnostics}
        onCursor={(line, column) => setCursor([line, column])}
      />
      <section aria-label="Problems" className="border-t border-line-subtle">
        <header className="flex h-8 items-center gap-1.5 px-4 text-[11px] font-medium text-primary">
          Problems
          <span className="rounded-full bg-input px-1.5 font-normal text-muted tabular-nums">
            {diagnostics?.length ?? 0}
          </span>
        </header>
        <ul className="max-h-72 overflow-y-auto pb-1.5">
          {diagnostics?.length === 0 && <li className="px-4 py-1 text-[11px] text-muted">No problems</li>}
          {diagnostics?.map((d) => {
            const isOpen = open === key(d);
            return (
              <li key={key(d)} className={isOpen ? "bg-hover/40" : ""}>
                <button
                  type="button"
                  aria-expanded={isOpen}
                  onClick={() => choose(d)}
                  className="flex h-7 w-full cursor-pointer items-center gap-2 px-4 text-left text-[12px] hover:bg-hover/50"
                >
                  <SeverityIcon severity={d.severity} />
                  <span className="min-w-0 truncate text-primary">{d.message}</span>
                  <span className="shrink-0 text-muted">Feanor {d.code}</span>
                  <span className="ml-auto shrink-0 pl-3 font-mono text-[11px] text-muted tabular-nums">
                    {d.start.line + 1}:{d.start.character + 1}
                  </span>
                </button>
                {isOpen && (
                  <div className="flex flex-col gap-2 pr-4 pb-3 pl-[2.375rem]">
                    {d.fixes.length > 0 && (
                      <div className="flex flex-wrap gap-1.5">
                        {d.fixes.map((fix) => (
                          <button
                            key={fix.title}
                            type="button"
                            onClick={() => {
                              editor.current?.applyEdits(fix.edits);
                              setOpen(null);
                            }}
                            title={fix.applicability === "unsafe" ? "Unsafe: may change what the font does" : undefined}
                            className="h-6 cursor-pointer rounded border border-line-subtle bg-surface-raised px-2 text-[12px] text-primary transition-colors hover:bg-input"
                          >
                            {fix.title}
                            {fix.applicability === "unsafe" && <span className="text-muted"> · unsafe</span>}
                          </button>
                        ))}
                      </div>
                    )}
                    {explanations[d.code]}
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      </section>
    </AppWindow>
  );
}
