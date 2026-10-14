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
  /** The first paragraph of "What it reports", on one line. */
  summary: string;
  /** The first ```fea block under "Example", if the rule has one. */
  example: string | null;
  /** The documentation without its heading and severity line. */
  body: string;
}

function firstParagraph(markdown: string, heading: string): string {
  const after = markdown.split(`## ${heading}`)[1] ?? "";
  return after
    .trim()
    .split(/\n\s*\n/)[0]
    .replace(/\s+/g, " ");
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
      const example =
        body
          .split("## Example")[1]
          ?.match(/```fea\n([\s\S]*?)```/)?.[1]
          .trimEnd() ?? null;
      return { code, name, severity, fix, summary: firstParagraph(body, "What it reports"), example, body };
    })
    .sort((a, b) => a.code.localeCompare(b.code));
}
