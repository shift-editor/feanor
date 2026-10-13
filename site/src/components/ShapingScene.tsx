"use client";

import { useEffect, useRef, useState } from "react";
import { hatch } from "@/components/HatchedBars";
import { type Diagnostic, loadFeanor } from "@/lib/feanor";

/** A glyph as HarfBuzz placed it: its outline in font units and its pen position. */
interface ShapedGlyph {
  name: string;
  d: string;
  x: number;
  y: number;
  advance: number;
}

interface Variant {
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

interface Props {
  scene: SceneData;
  /** Which pair's kerning to show as a hatched band: the glyph index of its first glyph. */
  pair: number;
  /** Each version of the feature code, highlighted on the server. */
  code: Record<Which, React.ReactNode>;
  labels: { written: string; meant: string };
  /** What changed in the fixed version, shown in its place of the diagnostic. */
  fixed: string;
}

type Which = "written" | "meant";

// Shift's negative-spacing amber, as the spacing tool draws it
const AMBER = "217, 119, 6";
const SWITCH_EVERY_MS = 2600;

/**
 * Shows what a mistake does to set text: the same words shaped by HarfBuzz from
 * the feature code as written and as meant, animating between the two.
 */
export function ShapingScene({ scene, pair, code, labels, fixed }: Props) {
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
        const found = feanor.lint(scene.written.fea, {}).find((d) => d.code.startsWith("R") || d.code.startsWith("E"));
        if (!cancelled) setDiagnostic(found ?? null);
      } catch {
        // the scene still shows without its diagnostic
      }
    }
    lintWritten();
    return () => {
      cancelled = true;
    };
  }, [scene.written.fea]);

  const glyphs = scene[which].glyphs;
  const width = Math.max(
    ...(["written", "meant"] as const).map((w) => {
      const g = scene[w].glyphs;
      return g[g.length - 1].x + g[g.length - 1].advance;
    }),
  );
  const pad = scene.unitsPerEm * 0.08;
  const top = scene.ascender;
  const height = scene.ascender - scene.descender;
  const viewBox = `${-pad} ${-top} ${width + 2 * pad} ${height}`;
  const toPercent = (x: number) => `${((x + pad) / (width + 2 * pad)) * 100}%`;

  // the kerning on the pair: from where the second glyph sits to where it would sit unkerned
  const first = glyphs[pair];
  const second = glyphs[pair + 1];
  const unkerned = first.x + scene.widths[first.name];
  const kern = second.x - unkerned;
  const bandLeft = Math.min(second.x, unkerned);
  const bandWidth = Math.abs(kern);

  return (
    <figure
      className="grid gap-6 rounded-xl border border-line bg-surface-raised p-5 lg:grid-cols-[minmax(0,26rem)_minmax(0,1fr)]"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
    >
      <div className="flex min-w-0 flex-col gap-3">
        {code[which]}
        {which === "meant" && <p className="font-copy text-sm text-secondary">{fixed}</p>}
        {which === "written" && diagnostic && (
          <p className="flex gap-2 font-ui text-sm">
            <span className="h-fit rounded bg-[#fdf3dc] px-1.5 py-0.5 font-mono text-[11px] text-[#8a5a00]">
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
      </div>

      <div className="flex min-w-0 flex-col justify-center gap-4">
        <div className="relative">
          <div
            aria-hidden
            className="absolute top-0 bottom-0 transition-all duration-700 ease-in-out"
            style={{ left: toPercent(bandLeft), width: `${(bandWidth / (width + 2 * pad)) * 100}%`, ...hatch(AMBER, 45) }}
          />
          <svg
            viewBox={viewBox}
            role="img"
            aria-label={`${scene.text}, ${which === "written" ? labels.written : labels.meant}`}
            className="relative block w-full"
          >
            {glyphs.map((g, i) => (
              <path
                key={i}
                d={g.d}
                className="fill-primary transition-transform duration-700 ease-in-out"
                style={{ transform: `translate(${g.x}px, ${-g.y}px) scale(1, -1)` }}
              />
            ))}
          </svg>
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
                {w === "written" ? "As written" : "Fixed"}
              </button>
            ))}
          </div>
          <span className="font-mono text-xs text-secondary tabular-nums">
            {which === "written" ? labels.written : labels.meant} · {first.name} {second.name} {kern > 0 ? "+" : "−"}
            {Math.abs(kern)}
          </span>
        </div>
        <p className="font-copy text-xs text-muted">
          Shaped by HarfBuzz from each version of the code. {scene.font}.
        </p>
      </div>
    </figure>
  );
}
