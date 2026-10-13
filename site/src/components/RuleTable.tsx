"use client";

import { Tabs } from "@base-ui/react/tabs";
import { Plus } from "lucide-react";
import { useState } from "react";

export interface RuleRow {
  code: string;
  name: string;
  severity: string;
  fix: string;
  /** What it reports and the rest of the documentation, rendered on the server. */
  doc: React.ReactNode;
}

/** What each code letter covers. */
const FAMILIES: Record<string, string> = {
  C: "Classes",
  L: "Lookups",
  G: "Glyphs",
  T: "Feature tags",
  X: "Characters",
  E: "Syntax",
  P: "Projects",
};

/** Code · Rule · Severity · Fix · open, shared by the header and every row. */
const COLUMNS =
  "grid grid-cols-[4.5rem_minmax(0,1fr)_2rem] items-center gap-x-3 px-3 sm:grid-cols-[5rem_minmax(0,1fr)_7rem_7rem_2rem]";

/** "error (or warning, as fea-rs reports it)" → "Error" */
function word(text: string) {
  const w = text.split(/[\s(]/)[0];
  return w[0].toUpperCase() + w.slice(1);
}

/**
 * Every rule, filtered by family, each row opening onto its documentation.
 * After the Variation panel in Shift: zebra rows, grey chips, pill tabs.
 */
export function RuleTable({ rules }: { rules: RuleRow[] }) {
  const families = Object.keys(FAMILIES).filter((f) => rules.some((r) => r.code.startsWith(f)));
  const [family, setFamily] = useState("all");
  const shown = family === "all" ? rules : rules.filter((r) => r.code.startsWith(family));

  return (
    <div className="flex flex-col gap-3 font-ui">
      <Tabs.Root value={family} onValueChange={(v) => setFamily(v as string)}>
        <Tabs.List className="flex flex-wrap gap-1 border-b border-line-subtle pb-3">
          {["all", ...families].map((f) => (
            <Tabs.Tab
              key={f}
              value={f}
              className="cursor-pointer rounded-md px-2.5 py-1 text-[15px] text-primary outline-none hover:bg-hover/50 focus-visible:ring-2 focus-visible:ring-primary/50 data-[active]:bg-hover"
            >
              {f === "all" ? "All" : FAMILIES[f]}
            </Tabs.Tab>
          ))}
        </Tabs.List>
      </Tabs.Root>

      <div>
        <div aria-hidden className={`${COLUMNS} py-2 text-[15px] font-medium`}>
          <span>Code</span>
          <span>Rule</span>
          <span className="hidden text-center sm:block">
            Severity
          </span>
          <span className="hidden text-center sm:block">
            Fix
          </span>
          <span />
        </div>
        {shown.map((r, i) => (
          <details key={r.code} className={`group rounded-md ${i % 2 === 0 ? "bg-stripe" : ""}`}>
            <summary className={`${COLUMNS} cursor-pointer list-none py-2 text-[15px] [&::-webkit-details-marker]:hidden`}>
              <span>
                <span className="rounded bg-chrome px-1.5 py-0.5 font-mono text-[12px] text-primary">{r.code}</span>
              </span>
              <span className="truncate text-primary">{r.name}</span>
              <span className="hidden text-center sm:block">{word(r.severity)}</span>
              <span className="hidden text-center sm:block">{r.fix === "none" ? "—" : word(r.fix)}</span>
              <span
                aria-hidden
                className="grid size-6 place-items-center rounded text-secondary transition-colors group-hover:bg-hover group-hover:text-primary group-open:bg-hover group-open:text-primary"
              >
                <Plus className="size-3.5 transition-transform group-open:rotate-45" strokeWidth={2} />
              </span>
            </summary>
            <div className="px-3 pt-1 pb-5 sm:pl-[calc(5rem+1.5rem)]">
              <div className="max-w-[64ch]">{r.doc}</div>
            </div>
          </details>
        ))}
      </div>
    </div>
  );
}
