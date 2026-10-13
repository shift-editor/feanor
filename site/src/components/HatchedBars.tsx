"use client";

import { useState } from "react";

/**
 * A horizontal bar chart filled with the hatching Shift draws for spacing:
 * 1px diagonal lines every 6px of width over a light tint of the same colour.
 * Each series hatches at 45° or 135°, as the two sides of a spacing gap do, so
 * series differ by direction as well as colour.
 */

export interface Series {
  key: string;
  name: string;
  /** An `r, g, b` triple. */
  rgb: string;
  angle: 45 | 135;
}

export interface Row {
  label: string;
  /** A second line under the label, such as the file's size. */
  detail: string;
  values: Record<string, number>;
}

interface Props {
  title: string;
  series: Series[];
  rows: Row[];
  /** How to print a value, e.g. 1.5 → "1.5 ms". */
  format: (value: number) => string;
  /** The axis label, under the ticks. */
  axis: string;
}

/**
 * Shift's spacing hatch as CSS backgrounds: a 6px tile holding one diagonal,
 * so lines land on whole pixels and every bar hatches alike.
 */
export function hatch(rgb: string, angle: 45 | 135): React.CSSProperties {
  // 45° rises to the right, like the left half of a spacing gap; 135° mirrors it
  const line = angle === 45 ? "M-1 1 1-1M0 6 6 0M5 7 7 5" : "M-1 5 1 7M0 0 6 6M5-1 7 1";
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="6" height="6"><path d="${line}" stroke="rgb(${rgb})" stroke-opacity="0.75" stroke-width="1" shape-rendering="crispEdges"/></svg>`;
  return {
    backgroundColor: `rgba(${rgb}, 0.14)`,
    backgroundImage: `url("data:image/svg+xml,${encodeURIComponent(svg)}")`,
    backgroundSize: "6px 6px",
  };
}

/** A round step giving four or five ticks up to `max`. */
function niceStep(max: number) {
  const raw = max / 4;
  const power = 10 ** Math.floor(Math.log10(raw));
  return [1, 2, 2.5, 5, 10].map((m) => m * power).find((s) => s >= raw) ?? raw;
}

function Gridlines({ ticks, top }: { ticks: number[]; top: number }) {
  return (
    <div aria-hidden className="pointer-events-none absolute inset-0">
      {ticks.map((t) => (
        <div key={t} className="absolute inset-y-0 w-px bg-line-subtle" style={{ left: `${(t / top) * 100}%` }} />
      ))}
    </div>
  );
}

export function HatchedBars({ title, series, rows, format, axis }: Props) {
  const [hovered, setHovered] = useState<number | null>(null);
  const max = Math.max(...rows.flatMap((r) => series.map((s) => r.values[s.key] ?? 0)));
  const step = niceStep(max);
  const top = Math.ceil(max / step) * step;
  const ticks = Array.from({ length: Math.round(top / step) + 1 }, (_, i) => i * step);

  return (
    <figure className="flex min-w-0 flex-col gap-4">
      <figcaption className="flex flex-wrap items-baseline justify-between gap-x-6 gap-y-2">
        <span className="font-release-heading text-lg font-bold tracking-[-0.03em]">{title}</span>
        <span className="flex flex-wrap gap-x-4 gap-y-1 font-ui text-xs text-secondary">
          {series.map((s) => (
            <span key={s.key} className="flex items-center gap-1.5">
              <span aria-hidden className="inline-block size-3" style={hatch(s.rgb, s.angle)} />
              {s.name}
            </span>
          ))}
        </span>
      </figcaption>

      <div>
        {rows.map((row, i) => (
          <div
            key={row.label}
            className="relative grid grid-cols-1 gap-x-4 sm:grid-cols-[10rem_minmax(0,1fr)]"
            onMouseEnter={() => setHovered(i)}
            onMouseLeave={() => setHovered(null)}
          >
            <div className="flex flex-col justify-center pt-3 font-ui text-xs leading-tight sm:py-2.5">
              <span className="text-primary">{row.label}</span>
              <span className="font-mono text-[11px] text-muted">{row.detail}</span>
            </div>
            <div className="relative mr-20 flex flex-col justify-center gap-[3px] py-2.5">
              <Gridlines ticks={ticks} top={top} />
              {series.map((s) => {
                const v = row.values[s.key] ?? 0;
                return (
                  <div key={s.key} className="relative flex h-4 items-center">
                    <div
                      className="h-full shrink-0"
                      style={{ width: `max(3px, ${(v / top) * 100}%)`, ...hatch(s.rgb, s.angle) }}
                    />
                    <span className="ml-2 font-mono text-[11px] whitespace-nowrap text-secondary tabular-nums">
                      {format(v)}
                    </span>
                  </div>
                );
              })}
            </div>
            {hovered === i && (
              <div
                role="tooltip"
                className="pointer-events-none absolute top-[85%] right-20 z-10 border border-primary bg-surface-raised px-3 py-2 font-ui text-xs"
              >
                <p className="font-medium text-primary">
                  {row.label} <span className="font-mono font-normal text-muted">{row.detail}</span>
                </p>
                <dl className="mt-1.5 grid grid-cols-[auto_auto] gap-x-5 gap-y-0.5 text-secondary">
                  {series.map((s) => (
                    <div key={s.key} className="contents">
                      <dt className="flex items-center gap-1.5">
                        <span aria-hidden className="inline-block size-2.5" style={hatch(s.rgb, s.angle)} />
                        {s.name}
                      </dt>
                      <dd className="text-right font-mono tabular-nums">{format(row.values[s.key] ?? 0)}</dd>
                    </div>
                  ))}
                </dl>
              </div>
            )}
          </div>
        ))}

        {/* axis */}
        <div className="grid grid-cols-1 gap-x-4 sm:grid-cols-[10rem_minmax(0,1fr)]">
          <span className="hidden sm:block" />
          <div className="relative mr-20 h-5 border-t border-line">
            {ticks.map((t) => (
              <span
                key={t}
                className="absolute top-1 -translate-x-1/2 font-mono text-[10px] text-muted tabular-nums"
                style={{ left: `${(t / top) * 100}%` }}
              >
                {t}
              </span>
            ))}
          </div>
          <span className="hidden sm:block" />
          <span className="mt-1 font-ui text-[11px] text-muted">{axis}</span>
        </div>
      </div>

      {/* the same numbers for screen readers (a table ignores sr-only, so wrap it) */}
      <div className="sr-only">
      <table>
          <caption>{title}</caption>
          <thead>
            <tr className="border-b border-line">
              <th className="py-1 pr-4 font-medium">File</th>
              <th className="py-1 pr-4 font-medium">Size</th>
              {series.map((s) => (
                <th key={s.key} className="py-1 pr-4 text-right font-medium">
                  {s.name}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="font-mono tabular-nums">
            {rows.map((r) => (
              <tr key={r.label} className="border-b border-line-subtle">
                <td className="py-1 pr-4 font-ui">{r.label}</td>
                <td className="py-1 pr-4">{r.detail}</td>
                {series.map((s) => (
                  <td key={s.key} className="py-1 pr-4 text-right">
                    {format(r.values[s.key] ?? 0)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </figure>
  );
}
