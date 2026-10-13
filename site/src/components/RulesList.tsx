import ReactMarkdown from "react-markdown";
import { RuleDoc } from "@/components/RuleDoc";
import type { RuleDoc as Rule } from "@/lib/content";

const FAMILY: Record<string, string> = {
  E: "Syntax",
  G: "Glyphs",
  C: "Classes",
  L: "Lookups",
  T: "Tags",
  X: "Text",
  P: "Project",
};

const SEVERITY: Record<string, string> = {
  error: "bg-danger-surface text-danger",
  warning: "bg-[#fdf3dc] text-[#8a5a00]",
  hint: "bg-surface-muted text-muted",
};

/** The first word of a "Severity" or "Fix" line: `error`, `safe`, `none`… */
const kind = (text: string) => text.split(/[\s(;]/)[0].toLowerCase();

function Chip({ className, children }: { className: string; children: React.ReactNode }) {
  return <span className={`rounded px-1.5 py-0.5 font-mono text-[11px] leading-none ${className}`}>{children}</span>;
}

/** Every rule as a card: what it finds, how serious, whether it fixes, and its documentation. */
export function RulesList({ rules }: { rules: Rule[] }) {
  return (
    <ul className="grid grid-cols-[minmax(0,1fr)] gap-3 md:grid-cols-2">
      {rules.map((r) => {
        const severity = kind(r.severity);
        const fix = kind(r.fix);
        return (
          <li key={r.code} id={`rule-${r.code}`} className="flex flex-col gap-2 rounded-xl border border-line bg-surface-raised p-5">
            <div className="flex items-center gap-2 font-mono text-xs text-muted">
              <span className="text-primary">{r.code}</span>
              <span aria-hidden>·</span>
              <span>{FAMILY[r.code[0]] ?? ""}</span>
              <span className="ml-auto flex gap-1.5">
                <Chip className={SEVERITY[severity] ?? SEVERITY.hint}>{severity}</Chip>
                {fix !== "none" && <Chip className="border border-line text-secondary">{fix} fix</Chip>}
              </span>
            </div>
            <h3 className="font-release-heading text-xl font-bold tracking-[-0.04em]">{r.name}</h3>
            <div className="font-copy text-[15px] leading-relaxed text-secondary [&_code]:rounded [&_code]:bg-surface-muted [&_code]:px-1 [&_code]:font-mono [&_code]:text-[12px] [&_p]:m-0">
              <ReactMarkdown>{r.summary}</ReactMarkdown>
            </div>
            <details className="group mt-1">
              <summary className="flex cursor-pointer list-none items-center gap-1 font-ui text-xs font-medium text-accent hover:underline [&::-webkit-details-marker]:hidden">
                How it works
                <span aria-hidden className="transition-transform group-open:rotate-90">
                  ›
                </span>
              </summary>
              <div className="mt-3 border-t border-line-subtle pt-3">
                <RuleDoc markdown={r.details} />
              </div>
            </details>
          </li>
        );
      })}
    </ul>
  );
}
