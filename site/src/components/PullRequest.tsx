import { CircleX } from "lucide-react";
import { AppWindow } from "./Window";

type Line = { old?: number; new?: number; kind: "context" | "add"; text: string };

// A pull request that adds tabular figures, with one mistake in it
const LINES: Line[] = [
  { old: 68, new: 68, kind: "context", text: "} pnum;" },
  { old: 69, new: 69, kind: "context", text: "" },
  { new: 70, kind: "add", text: "feature tnum {" },
  { new: 71, kind: "add", text: "    sub @figures_prop by @figures_tab;" },
  { new: 72, kind: "add", text: "    sub @numbers by @numbers_dflt;" },
  { new: 73, kind: "add", text: "} tnum;" },
];
const FLAGGED = 72;

/**
 * What `feanor check --format github` looks like on a pull request: the
 * problem as an annotation on the changed line, and the check failing.
 */
export function PullRequest() {
  return (
    <AppWindow
      title="Add tabular figures #42"
      detail="Files changed"
      status={
        <>
          <CircleX aria-hidden className="size-3.5 text-[#dc2626]" />
          <span className="text-primary">Feature code / feanor</span>
          <span>Failing after 9s</span>
          <span className="flex-1" />
          <span>1 annotation</span>
        </>
      }
    >
      <div className="flex items-center gap-2 border-b border-line-subtle bg-[#fafafa] px-3 py-2 font-mono text-[12px]">
        <span className="text-primary">features/common.fea</span>
        <span className="ml-auto font-ui text-[11px] tabular-nums">
          <span className="text-[#16a34a]">+4</span> <span className="text-muted">−0</span>
        </span>
      </div>
      <div className="overflow-x-auto font-mono text-[12px] leading-[20px]">
        <div className="bg-[#eef6ff] px-3 text-muted">@@ -68,2 +68,6 @@ feature pnum</div>
        {LINES.map((l, i) => (
          <div key={i}>
            <div
              className={`grid grid-cols-[2.5rem_2.5rem_1.25rem_minmax(0,1fr)] whitespace-pre ${l.kind === "add" ? "bg-[#ecfdf0]" : ""}`}
            >
              <span className="pr-2 text-right text-[#9f9f9f] select-none">{l.old ?? ""}</span>
              <span className="pr-2 text-right text-[#9f9f9f] select-none">{l.new ?? ""}</span>
              <span className="text-center text-muted select-none">{l.kind === "add" ? "+" : ""}</span>
              <span className="text-primary">{l.text}</span>
            </div>
            {l.new === FLAGGED && <Annotation />}
          </div>
        ))}
      </div>
    </AppWindow>
  );
}

function Annotation() {
  return (
    <div className="border-y border-line-subtle bg-surface-raised px-3 py-2.5 font-ui">
      <div className="overflow-hidden rounded-md border border-[#fca5a5] whitespace-normal">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-0.5 border-b border-[#fecaca] bg-[#fef2f2] px-3 py-1.5 text-[12px]">
          <CircleX aria-hidden className="size-3.5 shrink-0 text-[#dc2626]" />
          <span className="font-medium text-primary">Check failure on line {FLAGGED} in features/common.fea</span>
          <span className="text-muted">Feature code / feanor</span>
        </div>
        <p className="px-3 py-2 font-mono text-[12px] text-primary">
          C003 undefined-class: class @numbers_dflt is never defined in the project
        </p>
      </div>
    </div>
  );
}
