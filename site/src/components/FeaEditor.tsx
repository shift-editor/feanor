"use client";

import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { type Diagnostic as CmDiagnostic, linter } from "@codemirror/lint";
import { EditorState, StateEffect, StateField } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  WidgetType,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
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
  /** The cursor's one-based line and column, as it moves. */
  onCursor?: (line: number, column: number) => void;
}

const ERROR = "#dc2626";
const WARNING = "#f97316";
const HINT = "#9f9f9f";

// Shift's feature code editor theme (featureCodeTheme.ts in the Shift app): a
// quiet gutter, compact mono type, dotted problem underlines, and each line's
// problem written at its end instead of a gutter icon.
const theme = EditorView.theme({
  "&": { fontSize: "13px", color: "var(--color-primary)", backgroundColor: "var(--color-surface-raised)" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    fontFamily: "var(--font-mono)",
    lineHeight: "21px",
    scrollbarWidth: "thin",
    scrollbarColor: "var(--color-control-muted) transparent",
  },
  ".cm-content": { padding: "8px 0", caretColor: "var(--color-accent)" },
  ".cm-line": { padding: "0 16px 0 8px" },
  ".cm-gutters": { backgroundColor: "var(--color-surface-raised)", color: HINT, border: "none", fontSize: "11px" },
  ".cm-lineNumbers .cm-gutterElement": { minWidth: "28px", padding: "0 4px 0 12px" },
  ".cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--color-hover) 45%, transparent)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--color-primary)" },
  ".cm-cursor": { borderLeftColor: "var(--color-accent)", borderLeftWidth: "1.5px" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
    backgroundColor: "color-mix(in srgb, var(--color-accent) 20%, transparent)",
  },
  ".cm-lintRange-error, .cm-lintRange-warning, .cm-lintRange-hint": {
    backgroundImage: "none",
    textDecoration: "underline dotted",
    textUnderlineOffset: "3px",
    textDecorationThickness: "1.5px",
  },
  ".cm-lintRange-error": { textDecorationColor: ERROR },
  ".cm-lintRange-warning": { textDecorationColor: WARNING },
  ".cm-lintRange-hint": { textDecorationColor: HINT },
  ".cm-fea-inline": { paddingLeft: "16px", fontFamily: "var(--font-ui)", fontSize: "11px" },
  ".cm-fea-inline-error": { color: ERROR },
  ".cm-fea-inline-warning": { color: WARNING },
  ".cm-fea-inline-hint": { color: "#898989" },
  ".cm-tooltip.cm-tooltip-hover:has(> .cm-tooltip-lint)": {
    maxWidth: "520px",
    padding: "0",
    backgroundColor: "var(--color-surface-raised)",
    border: "1px solid var(--color-line-subtle)",
    borderRadius: "6px",
    boxShadow: "0 4px 16px rgb(0 0 0 / 0.12)",
    fontFamily: "var(--font-ui)",
    fontSize: "12px",
  },
  // the border takes the worst problem's colour
  ".cm-tooltip.cm-tooltip-hover:has(.cm-diagnostic-warning)": {
    borderColor: `color-mix(in srgb, ${WARNING} 55%, transparent)`,
  },
  ".cm-tooltip.cm-tooltip-hover:has(.cm-diagnostic-error)": {
    borderColor: `color-mix(in srgb, ${ERROR} 55%, transparent)`,
  },
  ".cm-tooltip-lint .cm-diagnostic": {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "baseline",
    columnGap: "12px",
    rowGap: "4px",
    padding: "6px 10px",
    borderLeft: "none",
    whiteSpace: "normal",
  },
  ".cm-tooltip-lint .cm-diagnostic + .cm-diagnostic": { borderTop: "1px solid var(--color-line-subtle)" },
  ".cm-tooltip-lint .cm-diagnosticText": { flexBasis: "100%" },
  ".cm-diagnosticSource": { color: "var(--color-muted)" },
  ".cm-tooltip-lint .cm-diagnosticAction": {
    margin: "0",
    padding: "0",
    border: "none",
    borderRadius: "0",
    backgroundColor: "transparent",
    color: "var(--color-accent)",
    fontFamily: "var(--font-ui)",
    fontSize: "11px",
    cursor: "pointer",
  },
  ".cm-tooltip-lint .cm-diagnosticAction:hover": { textDecoration: "underline" },
});

/** A line's problem, written after its code. */
class InlineMessage extends WidgetType {
  constructor(
    readonly message: string,
    readonly severity: Diagnostic["severity"],
  ) {
    super();
  }
  eq(other: InlineMessage) {
    return other.message === this.message && other.severity === this.severity;
  }
  toDOM() {
    const span = document.createElement("span");
    span.className = `cm-fea-inline cm-fea-inline-${this.severity}`;
    span.textContent = this.message;
    return span;
  }
}

const RANK = { error: 0, warning: 1, hint: 2 } as const;
const setInline = StateEffect.define<DecorationSet>();
const inlineMessages = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(set, tr) {
    for (const e of tr.effects) if (e.is(setInline)) return e.value;
    return set.map(tr.changes);
  },
  provide: (f) => EditorView.decorations.from(f),
});

/** The worst problem on each line, as a widget at the line's end. */
function inline(state: EditorState, diagnostics: Diagnostic[]): DecorationSet {
  const worst = new Map<number, Diagnostic>();
  for (const d of diagnostics) {
    const line = state.doc.lineAt(Math.min(d.from, state.doc.length)).number;
    const seen = worst.get(line);
    if (!seen || RANK[d.severity] < RANK[seen.severity]) worst.set(line, d);
  }
  return Decoration.set(
    [...worst].map(([line, d]) =>
      Decoration.widget({ widget: new InlineMessage(d.message, d.severity), side: 1 }).range(state.doc.line(line).to),
    ),
    true,
  );
}

const SEVERITY = { error: "error", warning: "warning", hint: "hint" } as const;

/** A CodeMirror editor for feature code, linted live by Feanor as you type. */
export const FeaEditor = forwardRef<FeaEditorHandle, Props>(function FeaEditor(
  { initial, glyphs, height = "22rem", onChange, onDiagnostics, onCursor },
  ref,
) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const callbacks = useRef({ onChange, onDiagnostics, onCursor });
  callbacks.current = { onChange, onDiagnostics, onCursor };

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
        // after this update: the linter calls its source while CodeMirror may be mid-update
        queueMicrotask(() => {
          if (!disposed) v.dispatch({ effects: setInline.of(inline(v.state, found)) });
        });
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
          highlightActiveLineGutter(),
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          fea,
          feaHighlighting,
          lintSource,
          inlineMessages,
          theme,
          EditorView.theme({ "&": { height }, ".cm-scroller": { overflow: "auto" } }),
          EditorView.updateListener.of((update) => {
            if (update.selectionSet || update.docChanged) {
              const head = update.state.selection.main.head;
              const line = update.state.doc.lineAt(head);
              callbacks.current.onCursor?.(line.number, head - line.from + 1);
            }
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
    // line heights measured with the fallback font go stale once Paper Mono loads
    window.document.fonts?.ready.then(() => {
      if (!disposed) v.requestMeasure();
    });
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
