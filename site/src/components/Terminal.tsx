"use client";

import { Tabs } from "@base-ui/react/tabs";
import { useState } from "react";

/** One captured command: what was typed, what Feanor printed (with ANSI colours), and its exit status. */
export interface Session {
  id: string;
  label: string;
  command: string;
  output: string;
  status: number;
}

// Feanor's terminal colours, in the page's palette
const COLOURS: Record<number, string> = {
  31: "#dc2626", // errors, removed lines
  32: "#16a34a", // added lines
  33: "#c2410c", // warnings
  34: "#64748b", // gutters and arrows
  36: "#0e7490", // hints, hunk headers
};

interface Run {
  text: string;
  color?: string;
  bold: boolean;
  dim: boolean;
}

/** Split text with SGR escape codes into styled runs. */
function runs(text: string): Run[] {
  const out: Run[] = [];
  let style = { color: undefined as string | undefined, bold: false, dim: false };
  for (const part of text.split(/(\x1b\[[0-9;]*m)/)) {
    const code = part.match(/^\x1b\[([0-9;]*)m$/);
    if (!code) {
      if (part) out.push({ text: part, ...style });
      continue;
    }
    for (const n of (code[1] || "0").split(";").map(Number)) {
      if (n === 0) style = { color: undefined, bold: false, dim: false };
      else if (n === 1) style = { ...style, bold: true };
      else if (n === 2) style = { ...style, dim: true };
      else if (COLOURS[n]) style = { ...style, color: COLOURS[n] };
    }
  }
  return out;
}

/** Real Feanor sessions, one per tab. */
export function Terminal({ sessions }: { sessions: Session[] }) {
  const [active, setActive] = useState(0);
  const session = sessions[active];
  return (
    <div className="flex flex-col gap-4">
      <Tabs.Root value={active} onValueChange={(v) => setActive(v as number)}>
        <Tabs.List aria-label="Commands" className="flex flex-wrap gap-1 font-ui">
          {sessions.map((s, i) => (
            <Tabs.Tab
              key={s.id}
              value={i}
              className="cursor-pointer rounded-md px-2.5 py-1 text-[15px] text-primary outline-none hover:bg-hover/50 focus-visible:ring-2 focus-visible:ring-primary/50 data-[active]:bg-hover"
            >
              {s.label}
            </Tabs.Tab>
          ))}
        </Tabs.List>
      </Tabs.Root>
      <figure className="overflow-hidden rounded-xl border border-line bg-surface-raised">
        <pre className="max-h-[32rem] overflow-auto p-5 font-mono text-[13px] leading-[1.55] text-primary">
          <span className="text-secondary select-none">$ </span>
          <span className="font-semibold">{session.command}</span>
          {"\n"}
          {runs(session.output).map((r, i) => (
            <span
              key={i}
              style={{ color: r.color, fontWeight: r.bold ? 600 : undefined, opacity: r.dim ? 0.6 : undefined }}
            >
              {r.text}
            </span>
          ))}
          {"\n"}
          <span className="text-secondary select-none">exit status {session.status}</span>
        </pre>
      </figure>
    </div>
  );
}
