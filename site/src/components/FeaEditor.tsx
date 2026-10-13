"use client";

import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { type Diagnostic as CmDiagnostic, lintGutter, linter } from "@codemirror/lint";
import { EditorState } from "@codemirror/state";
import { EditorView, highlightActiveLine, keymap, lineNumbers } from "@codemirror/view";
import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { fea, feaHighlighting } from "@/lib/fea-language";
import { type Diagnostic, loadFeanor } from "@/lib/feanor";

export interface FeaEditorHandle {
  text(): string;
  /** Replace the whole text (one undoable change). */
  setText(text: string): void;
  /** Apply edits given as UTF-16 ranges into the current text. */
  applyEdits(edits: { from: number; to: number; insert: string }[]): void;
}

interface Props {
  initial: string;
  /** The font's glyph names; enables the glyph-not-in-font rule. */
  glyphs?: string[];
  /** Height of the editor, as CSS. */
  height?: string;
  onChange?: (text: string) => void;
  onDiagnostics?: (diagnostics: Diagnostic[]) => void;
}

const theme = EditorView.theme({
  "&": { fontSize: "13px", backgroundColor: "var(--color-surface-raised)" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--font-mono)", lineHeight: "1.6" },
  ".cm-gutters": { backgroundColor: "var(--color-surface)", color: "var(--color-placeholder)", border: "none" },
  ".cm-activeLine": { backgroundColor: "#f5f7fb" },
  ".cm-activeLineGutter": { backgroundColor: "#eef1f6" },
  ".cm-tooltip": { fontFamily: "var(--font-ui)", fontSize: "13px" },
  ".cm-diagnostic": { padding: "4px 8px" },
  ".cm-diagnosticAction": { fontFamily: "var(--font-ui)", borderRadius: "4px", backgroundColor: "var(--color-accent)" },
});

const SEVERITY = { error: "error", warning: "warning", hint: "hint" } as const;

/** A CodeMirror editor for feature code, linted live by Feanor as you type. */
export const FeaEditor = forwardRef<FeaEditorHandle, Props>(function FeaEditor(
  { initial, glyphs, height = "22rem", onChange, onDiagnostics },
  ref,
) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const callbacks = useRef({ onChange, onDiagnostics });
  callbacks.current = { onChange, onDiagnostics };

  useImperativeHandle(ref, () => ({
    text: () => view.current?.state.doc.toString() ?? "",
    setText(text) {
      const v = view.current;
      if (v) v.dispatch({ changes: { from: 0, to: v.state.doc.length, insert: text } });
    },
    applyEdits(edits) {
      view.current?.dispatch({ changes: edits });
    },
  }));

  useEffect(() => {
    if (!host.current) return;
    // Feanor's incremental document, kept in step with the editor's text
    let document: InstanceType<Awaited<ReturnType<typeof loadFeanor>>["Document"]> | null = null;
    let disposed = false;

    const lintSource = linter(
      async (v) => {
        const feanor = await loadFeanor();
        if (disposed) return [];
        document ??= new feanor.Document(v.state.doc.toString(), { glyphs });
        const found = document.diagnostics();
        callbacks.current.onDiagnostics?.(found);
        return found.map(
          (d): CmDiagnostic => ({
            from: d.from,
            to: Math.max(d.to, d.from),
            severity: SEVERITY[d.severity],
            source: d.code,
            message: `${d.message}`,
            actions: d.fixes.map((fix) => ({
              name: fix.applicability === "safe" ? fix.title : `${fix.title} (unsafe)`,
              apply(target) {
                target.dispatch({ changes: fix.edits });
              },
            })),
          }),
        );
      },
      { delay: 60 },
    );

    const v = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: initial,
        extensions: [
          lineNumbers(),
          highlightActiveLine(),
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          fea,
          feaHighlighting,
          lintGutter(),
          lintSource,
          theme,
          EditorView.theme({ "&": { height }, ".cm-scroller": { overflow: "auto" } }),
          EditorView.updateListener.of((update) => {
            if (!update.docChanged) return;
            if (document) {
              // positions in a transaction refer to the text before it, so apply right to left
              const changes: [number, number, string][] = [];
              update.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
                changes.push([fromA, toA, inserted.toString()]);
              });
              for (const [from, to, insert] of changes.reverse()) document.edit(from, to, insert);
            }
            callbacks.current.onChange?.(update.state.doc.toString());
          }),
        ],
      }),
    });
    view.current = v;
    return () => {
      disposed = true;
      document?.free();
      v.destroy();
      view.current = null;
    };
    // the editor owns its text after mount; `initial` and `glyphs` only seed it
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return <div ref={host} className="overflow-hidden" />;
});
