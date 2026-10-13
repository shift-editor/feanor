"use client";

import { HatchedBars, type Series } from "@/components/HatchedBars";
import { PER_EDIT, WHOLE_FILE } from "@/lib/benchmarks";

// One colour per measurement, the same in both charts: Shift's spacing magenta,
// its accent blue, and the amber it uses for negative spacing.
const LINT: Series = { key: "lint", name: "Lint the whole file", rgb: "219, 46, 145", angle: 45 };
const FORMAT: Series = { key: "format", name: "Format the whole file", rgb: "12, 146, 244", angle: 135 };
const EDIT: Series = { key: "edit", name: "Lint again after one edit", rgb: "217, 119, 6", angle: 135 };

const lines = (n: number) => `${n.toLocaleString("en")} lines`;
const ms = (v: number) => `${v < 10 ? v.toFixed(1) : Math.round(v)} ms`;

export function SpeedCharts() {
  return (
    <div className="flex flex-col gap-12">
      <HatchedBars
        title="Whole files"
        series={[LINT, FORMAT]}
        rows={WHOLE_FILE.map((f) => ({
          label: f.file,
          detail: lines(f.lines),
          values: { lint: f.lintMs, format: f.formatMs },
        }))}
        format={ms}
        axis="milliseconds"
      />
      <HatchedBars
        title="While you type"
        series={[LINT, EDIT]}
        rows={PER_EDIT.map((f) => ({
          label: f.file,
          detail: lines(f.lines),
          values: { lint: f.fullMs, edit: f.editMs },
        }))}
        format={ms}
        axis="milliseconds, median over 726 edits"
      />
    </div>
  );
}
