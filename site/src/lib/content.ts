// Read at build time: example files and the rule documentation.
import "server-only";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

const site = process.cwd();
const examples = path.join(site, "examples");
const rules = path.join(site, "..", "crates", "feanor", "rules");

export function example(file: string): string {
  return readFileSync(path.join(examples, file), "utf8");
}

export function glyphList(file: string): string[] {
  return example(file).split(/\s+/).filter(Boolean);
}

export function credit(file: string): string {
  const credits = JSON.parse(example("credits.json")) as Record<string, string>;
  return credits[file] ?? "";
}

export interface RuleDoc {
  code: string;
  name: string;
  severity: string;
  fix: string;
  /** The first paragraph of "What it reports" (Markdown, one line). */
  summary: string;
  /** The documentation after that paragraph. */
  details: string;
  /** The documentation without its heading and severity line. */
  body: string;
}

const WHAT_IT_REPORTS = "## What it reports";

/**
 * Split a rule's documentation into its one-paragraph summary (the start of
 * "What it reports") and everything after that paragraph.
 */
function splitSummary(body: string): { summary: string; details: string } {
  const start = body.indexOf(WHAT_IT_REPORTS);
  if (start < 0) return { summary: "", details: body };

  const after = body.slice(start + WHAT_IT_REPORTS.length).trimStart();
  const end = after.search(/\n\s*\n/);
  const paragraph = end < 0 ? after : after.slice(0, end);
  const rest = end < 0 ? "" : after.slice(end).trim();
  // keep the heading when the section continues past its first paragraph
  const details = rest.startsWith("## ") ? rest : `${WHAT_IT_REPORTS}\n\n${rest}`;
  return { summary: paragraph.replace(/\s+/g, " ").trim(), details };
}

/** The rule pages in crates/feanor/rules, the same text `feanor rule` prints. */
export function ruleDocs(): RuleDoc[] {
  return readdirSync(rules)
    .filter((f) => f.endsWith(".md"))
    .map((f) => {
      const text = readFileSync(path.join(rules, f), "utf8");
      const [heading, , meta, ...rest] = text.split("\n");
      const [code, name] = heading.replace(/^#\s+/, "").split(/\s+/);
      const severity = meta.match(/\*\*Severity:\*\*\s*([^·]+)/)?.[1].trim() ?? "";
      const fix = meta.match(/\*\*Fix:\*\*\s*(.+)$/)?.[1].trim() ?? "";
      const body = rest.join("\n").trim();
      return { code, name, severity, fix, ...splitSummary(body), body };
    })
    .sort((a, b) => a.code.localeCompare(b.code));
}
