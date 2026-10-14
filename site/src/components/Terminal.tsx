"use client";

import { Tabs } from "@base-ui/react/tabs";
import { useState } from "react";
import { TrafficLights } from "./Window";

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

const DIRECTORY = "~/my-font";

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

/** A zsh-style prompt; after a failed command the arrow is red and shows its exit status. */
function Prompt({ status }: { status?: number }) {
  const failed = status !== undefined && status !== 0;
  return (
    <span className="select-none">
      <span className="font-semibold text-[#0e7490]">{DIRECTORY}</span>{" "}
      {failed && <span className="text-[#dc2626]">{status} </span>}
      <span className={failed ? "text-[#dc2626]" : "text-[#16a34a]"}>❯</span>{" "}
    </span>
  );
}

/** Real Feanor sessions in a terminal window, one per tab. */
export function Terminal({ sessions }: { sessions: Session[] }) {
  const [active, setActive] = useState(0);
  const session = sessions[active];
  return (
    <section
      aria-label="Terminal"
      className="flex flex-col overflow-hidden rounded-[10px] border border-black/15 bg-white font-ui shadow-[0_12px_40px_-12px_rgba(0,0,0,0.3),0_4px_12px_-6px_rgba(0,0,0,0.12)]"
    >
      <Tabs.Root value={active} onValueChange={(v) => setActive(v as number)}>
        <header className="grid h-9 shrink-0 grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center bg-[#f6f6f6] pl-3.5 select-none">
          <TrafficLights />
          <p className="truncate px-4 text-[12px] whitespace-nowrap">
            <span className="font-medium">my-font</span>
            <span className="text-muted"> — zsh</span>
          </p>
          <span />
        </header>
        <Tabs.List
          aria-label="Commands"
          className="flex border-y border-line-subtle bg-[#ececec] text-[12px] select-none"
        >
          {sessions.map((s, i) => (
            <Tabs.Tab
              key={s.id}
              value={i}
              className="flex-1 cursor-pointer truncate border-l border-line-subtle px-3 py-1.5 text-center text-muted outline-none first:border-l-0 hover:bg-black/[0.04] focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:ring-inset data-[active]:bg-white data-[active]:text-primary"
            >
              {s.label}
            </Tabs.Tab>
          ))}
        </Tabs.List>
      </Tabs.Root>
      <pre className="max-h-[32rem] overflow-auto px-4 pt-3 pb-4 font-mono text-[13px] leading-[1.55] text-primary">
        <Prompt />
        <span>{session.command}</span>
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
        <Prompt status={session.status} />
        <span aria-hidden className="inline-block h-[1.1em] w-[0.6em] translate-y-[0.2em] bg-primary/70" />
      </pre>
    </section>
  );
}
