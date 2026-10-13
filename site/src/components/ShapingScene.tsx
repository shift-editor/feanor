"use client";

import { useEffect, useRef, useState } from "react";
import { hatch } from "@/components/HatchedBars";
import { type Diagnostic, loadFeanor } from "@/lib/feanor";

/** A glyph as HarfBuzz placed it: its outline in font units, its pen position, its cluster. */
interface ShapedGlyph {
  name: string;
  d: string;
  x: number;
  y: number;
  advance: number;
  cluster: number;
}

interface Variant {
  label: string;
  note: string;
  fea: string;
  glyphs: ShapedGlyph[];
}

/** One scene's data, built by `scenes/build.py`. */
export interface SceneData {
  text: string;
  widths: Record<string, number>;
  unitsPerEm: number;
  ascender: number;
  descender: number;
  font: string;
  written: Variant;
  meant: Variant;
}

type Which = "written" | "meant";

export interface Scene {
  id: string;
  title: string;
  data: SceneData;
  /** Each version of the feature code, highlighted on the server. */
  code: Record<Which, React.ReactNode>;
  /** For kerning: the index of the pair's first glyph, to hatch the kerning between them. */
  pair?: number;
}

// Shift's negative-spacing amber, as the spacing tool draws it
const AMBER = "217, 119, 6";
const CHANGED: Record<Which, string> = { written: "#d97706", meant: "#0c92f4" };
const SWITCH_EVERY_MS = 2600;

/** Glyphs of `which` that the other version doesn't have at the same place. */
function changed(data: SceneData, which: Which): Set<number> {
  const other = data[which === "written" ? "meant" : "written"].glyphs;
  const there = new Set(other.map((g) => `${g.cluster}:${g.name}`));
  const out = new Set<number>();
  data[which].glyphs.forEach((g, i) => {
    if (!there.has(`${g.cluster}:${g.name}`)) out.add(i);
  });
  return out;
}

/** The scenes, one at a time, with tabs to choose. */
export function ShapingScenes({ scenes }: { scenes: Scene[] }) {
  const [active, setActive] = useState(0);
  return (
    <div className="flex flex-col gap-4">
      <div role="tablist" aria-label="Mistakes" className="flex flex-wrap gap-1.5">
        {scenes.map((s, i) => (
          <button
            key={s.id}
            type="button"
            role="tab"
            aria-selected={i === active}
            aria-pressed={i === active}
            className="segment"
            onClick={() => setActive(i)}
          >
            {s.title}
          </button>
        ))}
      </div>
      <ShapingScene key={scenes[active].id} scene={scenes[active]} />
    </div>
  );
}

/**
 * What a mistake does to set text: the same words shaped by HarfBuzz from the
 * feature code as written and as fixed, animating between the two.
 */
function ShapingScene({ scene }: { scene: Scene }) {
  const { data, code, pair } = scene;
  const [which, setWhich] = useState<Which>("written");
  const [paused, setPaused] = useState(false);
  const [diagnostic, setDiagnostic] = useState<Diagnostic | null>(null);
  const reducedMotion = useRef(false);

  useEffect(() => {
    reducedMotion.current = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }, []);

  useEffect(() => {
    if (paused || reducedMotion.current) return;
    const timer = setInterval(() => setWhich((w) => (w === "written" ? "meant" : "written")), SWITCH_EVERY_MS);
    return () => clearInterval(timer);
  }, [paused]);

  useEffect(() => {
    let cancelled = false;
    async function lintWritten() {
      try {
        const feanor = await loadFeanor();
        const found = feanor.lint(data.written.fea, {}).find((d) => d.severity !== "hint");
        if (!cancelled) setDiagnostic(found ?? null);
      } catch {
        // the scene still shows without its diagnostic
      }
    }
    lintWritten();
    return () => {
      cancelled = true;
    };
  }, [data.written.fea]);

  const end = (w: Which) => {
    const g = data[w].glyphs;
    return g[g.length - 1].x + g[g.length - 1].advance;
  };
  const width = Math.max(end("written"), end("meant"));
  const pad = data.unitsPerEm * 0.08;
  const total = width + 2 * pad;
  const viewBox = `${-pad} ${-data.ascender} ${total} ${data.ascender - data.descender}`;
  const toPercent = (x: number) => `${((x + pad) / total) * 100}%`;
  // the same glyphs in both versions only move; different glyphs crossfade
  const names = (w: Which) => data[w].glyphs.map((g) => g.name).join(" ");
  const sameGlyphs = names("written") === names("meant");

  const variant = data[which];
  const kerning = pair === undefined ? null : kerningBand(data, which, pair);
  const severityStyle =
    diagnostic?.severity === "error" ? "bg-danger-surface text-danger" : "bg-[#fdf3dc] text-[#8a5a00]";

  return (
    <figure
      className="grid gap-6 rounded-xl border border-line bg-surface-raised p-5 lg:grid-cols-[minmax(0,26rem)_minmax(0,1fr)]"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
    >
      <div className="flex min-w-0 flex-col gap-3">
        {code[which]}
        {which === "written" && diagnostic && (
          <p className="flex gap-2 font-ui text-sm">
            <span className={`h-fit rounded px-1.5 py-0.5 font-mono text-[11px] ${severityStyle}`}>
              {diagnostic.severity}
            </span>
            <span>
              <span className="font-mono text-[13px]">
                {diagnostic.code} {diagnostic.rule}
              </span>
              : {diagnostic.message}
            </span>
          </p>
        )}
        {variant.note && <p className="font-copy text-sm text-secondary">{variant.note}</p>}
      </div>

      <div className="flex min-w-0 flex-col justify-center gap-4">
        <div className="relative">
          {kerning && (
            <div
              aria-hidden
              className="absolute top-0 bottom-0 transition-all duration-700 ease-in-out"
              style={{
                left: toPercent(kerning.left),
                width: `${(kerning.width / total) * 100}%`,
                ...hatch(AMBER, 45),
              }}
            />
          )}
          {sameGlyphs ? (
            <svg
              viewBox={viewBox}
              role="img"
              aria-label={`${data.text}, ${variant.label}`}
              className="relative block w-full"
            >
              {variant.glyphs.map((g, i) => (
                <path
                  key={i}
                  d={g.d}
                  className="fill-primary transition-transform duration-700 ease-in-out"
                  style={{ transform: `translate(${g.x}px, ${-g.y}px) scale(1, -1)` }}
                />
              ))}
            </svg>
          ) : (
            <div className="relative">
              {(["written", "meant"] as const).map((w, layer) => (
                <Word
                  key={w}
                  glyphs={data[w].glyphs}
                  changed={changed(data, w)}
                  color={CHANGED[w]}
                  viewBox={viewBox}
                  label={`${data.text}, ${data[w].label}`}
                  visible={w === which}
                  stacked={layer > 0}
                />
              ))}
            </div>
          )}
        </div>

        <div className="flex flex-wrap items-center justify-between gap-3 font-ui text-sm">
          <div className="flex gap-1.5" role="group" aria-label="Show the font">
            {(["written", "meant"] as const).map((w) => (
              <button
                key={w}
                type="button"
                className="segment"
                aria-pressed={which === w}
                onClick={() => {
                  setPaused(true);
                  setWhich(w);
                }}
              >
                {data[w].label}
              </button>
            ))}
          </div>
          {kerning && (
            <span className="font-mono text-xs text-secondary tabular-nums">
              {kerning.pair} {kerning.value > 0 ? "+" : "−"}
              {Math.abs(kerning.value)}
            </span>
          )}
        </div>
        <p className="font-copy text-xs text-muted">Shaped by HarfBuzz from each version of the code. {data.font}.</p>
      </div>
    </figure>
  );
}

/** The kerning on a pair: from where its second glyph sits to where it would sit unkerned. */
function kerningBand(data: SceneData, which: Which, pair: number) {
  const glyphs = data[which].glyphs;
  const first = glyphs[pair];
  const second = glyphs[pair + 1];
  const unkerned = first.x + data.widths[first.name];
  const value = second.x - unkerned;
  return {
    pair: `${first.name} ${second.name}`,
    value,
    left: Math.min(second.x, unkerned),
    width: Math.abs(value),
  };
}

/** One version of the word; glyphs that differ from the other version are coloured. */
function Word({
  glyphs,
  changed,
  color,
  viewBox,
  label,
  visible,
  stacked,
}: {
  glyphs: ShapedGlyph[];
  changed: Set<number>;
  color: string;
  viewBox: string;
  label: string;
  visible: boolean;
  stacked: boolean;
}) {
  const placement = stacked ? "absolute inset-0" : "relative";
  return (
    <svg
      viewBox={viewBox}
      role="img"
      aria-label={label}
      aria-hidden={!visible}
      className={`block w-full text-primary transition-opacity duration-500 ease-in-out ${placement}`}
      style={{ opacity: visible ? 1 : 0 }}
    >
      {glyphs.map((g, i) => (
        <path
          key={i}
          d={g.d}
          fill={changed.has(i) ? color : "currentColor"}
          transform={`translate(${g.x} ${-g.y}) scale(1 -1)`}
        />
      ))}
    </svg>
  );
}
