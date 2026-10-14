"use client";

import { Tabs } from "@base-ui/react/tabs";
import { useEffect, useState } from "react";
import { Segmented } from "@/components/controls";
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
  /** The extent to zoom to when the difference is small: left, right, bottom, top, in font units. */
  focus?: number[];
  /** Right to left: the versions line up at the right, and spacing is read from there. */
  rtl?: boolean;
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
  /** How often the mistake turns up in real feature code, e.g. "Found in 4 Google Fonts families". */
  found?: string;
}

// Shift's negative-spacing amber, as the spacing tool draws it
const AMBER = "217, 119, 6";
const CHANGED: Record<Which, string> = { written: "#d97706", meant: "#0c92f4" };
// Shift's problem colours, as in the lint demo
const DOT = { error: "bg-[#dc2626]", warning: "bg-[#f97316]", hint: "bg-[#9f9f9f]" } as const;

const other = (w: Which): Which => (w === "written" ? "meant" : "written");

/**
 * Glyphs of `which` whose spacing from the glyph before them, in reading order,
 * differs in the other version (same glyphs, moved). Glyphs that only shift
 * along with an earlier one aren't counted.
 */
function moved(data: SceneData, which: Which): Set<number> {
  const here = data[which].glyphs;
  const there = data[other(which)].glyphs;
  const step = data.rtl ? 1 : -1;
  const out = new Set<number>();
  here.forEach((g, i) => {
    const before = i + step;
    if (!there[i] || before < 0 || before >= here.length) return;
    const dx = g.x - here[before].x;
    const dy = g.y - here[before].y;
    if (dx !== there[i].x - there[before].x || dy !== there[i].y - there[before].y) out.add(i);
  });
  return out;
}

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
      <Tabs.Root value={active} onValueChange={(v) => setActive(v as number)}>
        <Tabs.List aria-label="Mistakes" className="flex flex-wrap gap-1 font-ui">
          {scenes.map((s, i) => (
            <Tabs.Tab
              key={s.id}
              value={i}
              className="cursor-pointer rounded-md px-2.5 py-1 text-[15px] text-primary outline-none hover:bg-hover/50 focus-visible:ring-2 focus-visible:ring-primary/50 data-[active]:bg-hover"
            >
              {s.title}
            </Tabs.Tab>
          ))}
        </Tabs.List>
      </Tabs.Root>
      <ShapingScene key={scenes[active].id} scene={scenes[active]} />
    </div>
  );
}

/**
 * What a mistake does to set text: the same words shaped by HarfBuzz from the
 * feature code as written and as fixed; a toggle switches between the two.
 */
function ShapingScene({ scene }: { scene: Scene }) {
  const { data, code, pair } = scene;
  const [which, setWhich] = useState<Which>("written");
  const [diagnostic, setDiagnostic] = useState<Diagnostic | null>(null);
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
  const pad = data.unitsPerEm * (data.focus ? 0.15 : 0.08);
  const [from, to] = data.focus ?? [0, Math.max(end("written"), end("meant"))];
  const left = from - pad;
  const total = to - from + 2 * pad;
  const [bottom, top] = data.focus ? [data.focus[2] - pad, data.focus[3] + pad] : [data.descender, data.ascender];
  const viewBox = `${left} ${-top} ${total} ${top - bottom}`;
  const toPercent = (x: number) => `${((x - left) / total) * 100}%`;
  // the same glyphs in both versions only move; different glyphs crossfade
  const names = (w: Which) => data[w].glyphs.map((g) => g.name).join(" ");
  const sameGlyphs = names("written") === names("meant");

  const variant = data[which];
  const kerning = pair === undefined ? null : kerningBand(data, which, pair);

  /** One version's code, with the problem Feanor finds in the written one, and its note. */
  const side = (w: Which) => (
    <>
      {code[w]}
      {w === "written" && diagnostic && (
        <div className="grid grid-cols-[0.5rem_minmax(0,1fr)] items-baseline gap-x-2.5 gap-y-1.5 rounded-md bg-stripe px-3 py-2.5 font-ui">
          <span aria-label={diagnostic.severity} className={`size-2 -translate-y-px rounded-full ${DOT[diagnostic.severity]}`} />
          <p className="text-[14px] leading-snug text-primary">{diagnostic.message}</p>
          <span
            title={diagnostic.rule}
            className="col-start-2 w-fit rounded bg-chrome px-1.5 py-0.5 font-mono text-[11px] text-primary"
          >
            {diagnostic.code}
          </span>
        </div>
      )}
      {data[w].note && <p className="font-copy text-sm text-secondary">{data[w].note}</p>}
    </>
  );

  return (
    <figure
      className="grid gap-6 rounded-xl border border-line bg-surface-raised p-5 lg:grid-cols-[minmax(0,26rem)_minmax(0,1fr)]"
    >
      {/* both versions share one grid cell, so the taller sets the height and toggling never shifts the layout */}
      <div className="grid min-w-0 [grid-template-areas:'stack']">
        {(["written", "meant"] as const).map((w) => (
          <div
            key={w}
            aria-hidden={w !== which}
            className={`flex min-w-0 flex-col gap-3 [grid-area:stack] ${w === which ? "" : "invisible"}`}
          >
            {side(w)}
          </div>
        ))}
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
                  className="fill-primary transition-[transform,fill] duration-700 ease-in-out"
                  style={{
                    transform: `translate(${g.x}px, ${-g.y}px) scale(1, -1)`,
                    ...(moved(data, which).has(i) && { fill: CHANGED[which] }),
                  }}
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
          <Segmented
            label="Show the font"
            value={which}
            onChange={setWhich}
            options={(["written", "meant"] as const).map((w) => ({ value: w, label: data[w].label }))}
          />
          {kerning && (
            <span className="font-mono text-xs text-secondary tabular-nums">
              {kerning.pair} {kerning.value > 0 ? "+" : "−"}
              {Math.abs(kerning.value)}
            </span>
          )}
        </div>
        {scene.found && <p className="font-copy text-xs text-muted">{scene.found}</p>}
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

