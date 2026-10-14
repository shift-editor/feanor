import ReactMarkdown from "react-markdown";
import { Fence } from "@/components/RuleDoc";
import { RuleTable } from "@/components/RuleTable";
import type { RuleDoc as Rule } from "@/lib/content";

const RULES_ON_GITHUB = "https://github.com/shift-editor/feanor/blob/main/crates/feanor/rules";

/** An opened row: what the rule reports, its example, and a link to the full page. */
export function Brief({ rule }: { rule: Rule }) {
  return (
    <div className="flex flex-col gap-3">
      <p className="font-copy text-[15px] leading-relaxed text-secondary [&_code]:font-mono [&_code]:text-[13px]">
        <ReactMarkdown components={{ p: ({ children }) => <>{children}</> }}>{rule.summary}</ReactMarkdown>
      </p>
      {rule.example && <Fence code={rule.example} />}
      <a
        href={`${RULES_ON_GITHUB}/${rule.code}.md`}
        className="self-start font-ui text-[13px] text-accent underline-offset-2 hover:underline"
      >
        Why it matters, the fix, and notes →
      </a>
    </div>
  );
}

/** One line on a rule, with a link to its page: for a problem opened in the lint demo. */
export function RuleSummary({ rule }: { rule: Rule }) {
  return (
    <p className="font-ui text-[12px] leading-relaxed text-secondary [&_code]:font-mono [&_code]:text-[11px]">
      <span className="font-medium text-primary">{rule.name}</span>{" "}
      <ReactMarkdown components={{ p: ({ children }) => <>{children}</> }}>{rule.summary}</ReactMarkdown>{" "}
      <a href={`${RULES_ON_GITHUB}/${rule.code}.md`} className="text-accent underline-offset-2 hover:underline">
        More →
      </a>
    </p>
  );
}

/** The rules, briefed here on the server, in a table that filters and opens on the client. */
export function RuleList({ rules }: { rules: Rule[] }) {
  return (
    <RuleTable
      rules={rules.map((r) => ({
        code: r.code,
        name: r.name,
        severity: r.severity,
        fix: r.fix,
        doc: <Brief rule={r} />,
      }))}
    />
  );
}
