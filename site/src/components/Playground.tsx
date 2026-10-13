"use client";

import { useRef, useState } from "react";
import { DiffView } from "./DiffView";
import { FeaEditor, type FeaEditorHandle } from "./FeaEditor";
import { AppWindow, BarButton, ProblemCount } from "./Window";
import { type Diagnostic, loadFeanor } from "@/lib/feanor";

interface Props {
  initial: string;
  glyphs: string[];
}

/** The hero: an editor linted as you type, a Format button, and the diff it would make. */
export function Playground({ initial, glyphs }: Props) {
  const editor = useRef<FeaEditorHandle>(null);
  const [diagnostics, setDiagnostics] = useState<Diagnostic[] | null>(null);
  const [diff, setDiff] = useState<{ before: string; after: string } | null>(null);
  const [cursor, setCursor] = useState<[number, number]>([1, 1]);

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
    <div className="flex flex-col gap-5">
      <AppWindow
        title="Feature Code"
        detail="features.fea"
        actions={
          <>
            <BarButton aria-pressed={diff !== null} onClick={diff ? () => setDiff(null) : showDiff}>
              Diff
            </BarButton>
            <button
              type="button"
              onClick={formatNow}
              className="h-6 cursor-pointer rounded bg-accent px-2 text-[11px] font-medium text-white transition-colors hover:bg-accent/90 focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:outline-none"
            >
              Format
            </button>
          </>
        }
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
          height="24rem"
          onDiagnostics={setDiagnostics}
          onChange={() => setDiff(null)}
          onCursor={(line, column) => setCursor([line, column])}
        />
        {diff && (
          <div className="border-t border-line-subtle">
            <DiffView name="features.fea" before={diff.before} after={diff.after} layout="unified" maxHeight="24rem" />
          </div>
        )}
      </AppWindow>
      <p className="font-ui text-xs text-muted">
        Problems are underlined, with the message at the end of the line: hover for any fixes. The glyph list for
        this example is {glyphs.length} names, so misspelled glyphs are caught too.
      </p>
    </div>
  );
}
