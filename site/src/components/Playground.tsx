"use client";

import { useRef, useState } from "react";
import { DiffView } from "./DiffView";
import { FeaEditor, type FeaEditorHandle } from "./FeaEditor";
import { type Diagnostic, loadFeanor } from "@/lib/feanor";

interface Props {
  initial: string;
  glyphs: string[];
}

function summary(diagnostics: Diagnostic[] | null) {
  if (!diagnostics) return "Loading Feanor…";
  if (diagnostics.length === 0) return "No problems found";
  const count = (s: Diagnostic["severity"]) => diagnostics.filter((d) => d.severity === s).length;
  const parts = [
    [count("error"), "error"],
    [count("warning"), "warning"],
    [count("hint"), "hint"],
  ].filter(([n]) => n) as [number, string][];
  return parts.map(([n, word]) => `${n} ${word}${n === 1 ? "" : "s"}`).join(" · ");
}

/** The hero: an editor linted as you type, a Format button, and the diff it would make. */
export function Playground({ initial, glyphs }: Props) {
  const editor = useRef<FeaEditorHandle>(null);
  const [diagnostics, setDiagnostics] = useState<Diagnostic[] | null>(null);
  const [diff, setDiff] = useState<{ before: string; after: string } | null>(null);

  async function showDiff() {
    const feanor = await loadFeanor();
    const before = editor.current?.text() ?? "";
    setDiff({ before, after: feanor.format(before, {}).text });
  }

  async function formatNow() {
    const feanor = await loadFeanor();
    const before = editor.current?.text() ?? "";
    editor.current?.setText(feanor.format(before, {}).text);
    setDiff(null);
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="overflow-hidden rounded-xl border border-line bg-surface-raised shadow-[0_1px_0_rgba(0,0,0,0.04),0_12px_32px_-16px_rgba(0,0,0,0.18)]">
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-line-subtle bg-surface px-3 py-2">
          <span className="font-mono text-xs text-muted">features.fea</span>
          <div className="flex items-center gap-2">
            <span className="font-ui text-xs text-muted" aria-live="polite">
              {summary(diagnostics)}
            </span>
            <button type="button" onClick={diff ? () => setDiff(null) : showDiff} className="button-quiet">
              {diff ? "Hide diff" : "Show diff"}
            </button>
            <button type="button" onClick={formatNow} className="button-primary">
              Format
            </button>
          </div>
        </div>
        <FeaEditor
          ref={editor}
          initial={initial}
          glyphs={glyphs}
          height="24rem"
          onDiagnostics={setDiagnostics}
          onChange={() => setDiff(null)}
        />
      </div>
      {diff && <DiffView name="features.fea" before={diff.before} after={diff.after} layout="unified" />}
      <p className="font-ui text-xs text-muted">
        Underlined code has a problem: hover it for the message and any fixes. The glyph list for this
        example is {glyphs.length} names, so misspelled glyphs are caught too.
      </p>
    </div>
  );
}
