import ReactMarkdown from "react-markdown";
import { CopyCommand } from "@/components/CopyCommand";
import { type Example, FormatDemo } from "@/components/FormatDemo";
import { LintDemo } from "@/components/LintDemo";
import { Playground } from "@/components/Playground";
import { Section } from "@/components/Section";
import { credit, example, glyphList, ruleDocs } from "@/lib/content";

const VERSION = "0.1.0";
const GITHUB = "https://github.com/shift-editor/feanor";

const LINKS = [
  { label: "GitHub", href: GITHUB },
  { label: "Docs", href: "https://docs.rs/feanor" },
  { label: "crates.io", href: "https://crates.io/crates/feanor-cli" },
];

const STATS = [
  { value: "2,762", label: "feature files from the upstream sources of every Google Fonts family" },
  { value: "0", label: "files that compile differently after formatting, checked with fea-rs and fontTools" },
  { value: "2,749 / 2,749", label: "planted mistakes found by the linter, at the right place" },
];

const CI = `- run: cargo install feanor-cli
- run: feanor check sources/features.fea --format github`;

const RUST = `use feanor::{format, lint, FormatOptions, LintOptions};

let formatted = format(source, &FormatOptions::default());
let diagnostics = lint(source, &LintOptions::default());`;

const JS = `import init, { format, lint } from "@shift-editor/feanor";

await init();
const diagnostics = lint(source, { glyphs });`;

function Code({ children, label }: { children: string; label?: string }) {
  return (
    <figure className="flex min-w-0 flex-col gap-1.5">
      {label && <figcaption className="font-ui text-xs text-muted">{label}</figcaption>}
      <pre className="overflow-x-auto rounded-lg border border-line bg-surface p-4 font-mono text-[13px] leading-relaxed">
        <code>{children}</code>
      </pre>
    </figure>
  );
}

export default function Home() {
  const examples: Example[] = [
    { id: "inter", label: "Inter: calt.fea", file: "inter-calt.fea" },
    { id: "recursive", label: "Recursive: common.fea", file: "recursive-common.fea" },
    { id: "nastaliq", label: "Noto Nastaliq Urdu: classes", file: "noto-nastaliq-classes.fea" },
  ].map((e) => ({ ...e, text: example(e.file), credit: credit(e.file) }));
  const glyphs = glyphList("playground-glyphs.txt");
  const rules = ruleDocs();

  return (
    <div className="mx-auto flex max-w-[68rem] flex-col gap-24 px-4 pb-20 sm:px-6">
      <header className="flex items-center justify-between pt-5 font-ui text-sm">
        <a href="#" className="font-mono text-base">
          feanor
        </a>
        <nav className="flex items-center gap-5 text-muted">
          {LINKS.map((l) => (
            <a key={l.label} href={l.href} className="hover:text-primary">
              {l.label}
            </a>
          ))}
          <a href="https://shift.graphics" className="hover:text-primary">
            by Shift
          </a>
        </nav>
      </header>

      <section className="flex flex-col gap-8">
        <div className="flex max-w-[44rem] flex-col gap-5">
          <h1 className="font-ui text-[2.5rem] font-bold leading-[1.05] tracking-[-0.05em] text-balance sm:text-display">
            Format and lint OpenType feature files
          </h1>
          <p className="max-w-[56ch] font-ui text-[17px] leading-relaxed text-secondary">
            Feanor is a fast formatter and linter for <code className="font-mono text-[15px]">.fea</code> code,
            written in Rust on fea-rs. Formatting only touches whitespace, so it never changes the compiled font.
            The linter finds misspelled glyphs, mismatched classes, and invisible characters, with fixes.
          </p>
          <div className="flex flex-wrap items-center gap-3">
            <CopyCommand command="cargo install feanor-cli" />
            <span className="rounded-full border border-line px-2.5 py-1 font-mono text-xs text-muted">v{VERSION}</span>
          </div>
        </div>
        <Playground initial={example("playground.fea")} glyphs={glyphs} />
      </section>

      <Section
        id="formatting"
        title="Formatting that leaves the font alone"
        lede={
          <p>
            Feanor changes spacing, indentation, line breaks and blank lines, and nothing else. Long glyph classes are
            packed to the line width, columns you aligned by hand stay aligned, and the file keeps its own indent and
            line endings. Try it on feature code from real fonts.
          </p>
        }
      >
        <FormatDemo examples={examples} />
      </Section>

      <Section
        id="linting"
        title="Mistakes, found where they are"
        lede={
          <p>
            Every diagnostic points at the exact text, explains itself, and often comes with a fix. Safe fixes can&apos;t
            change what the font does. Unsafe ones, like renaming a glyph to the closest match, wait for you. Edit the
            code and the list updates as you type.
          </p>
        }
      >
        <LintDemo initial={example("lint.fea")} glyphs={glyphs} />
      </Section>

      <Section
        id="rules"
        title="Rules"
        lede={
          <p>
            Each rule has a stable code and a page of documentation. Choose rules by code, family or name in{" "}
            <code className="font-mono text-[14px]">feanor.toml</code>, on the command line, or with a{" "}
            <code className="font-mono text-[14px]"># noqa</code> comment.
          </p>
        }
      >
        <div className="divide-y divide-line-subtle overflow-hidden rounded-xl border border-line">
          {rules.map((r) => (
            <details key={r.code} className="group bg-surface-raised open:bg-surface">
              <summary className="grid cursor-pointer grid-cols-[4.5rem_minmax(0,1fr)_auto] items-baseline gap-3 px-4 py-3 font-ui text-sm hover:bg-surface sm:grid-cols-[4.5rem_16rem_minmax(0,1fr)_auto]">
                <span className="font-mono text-xs text-muted">{r.code}</span>
                <span className="font-mono text-[13px]">{r.name}</span>
                <span className="hidden truncate text-muted sm:block">{r.fix === "none" ? "" : `fix: ${r.fix}`}</span>
                <span className="text-xs text-muted">{r.severity}</span>
              </summary>
              <div className="prose-rule max-w-[70ch] px-4 pb-5 pt-1">
                <ReactMarkdown>{r.body}</ReactMarkdown>
              </div>
            </details>
          ))}
        </div>
      </Section>

      <Section
        id="tested"
        title="Tested on all of Google Fonts"
        lede={
          <p>
            Every release is checked against the feature code in the source repository of every Google Fonts family.
            Each file is formatted, then compiled before and after with two independent compilers, which must produce
            identical layout tables.
          </p>
        }
      >
        <dl className="grid gap-4 sm:grid-cols-3">
          {STATS.map((s) => (
            <div key={s.label} className="flex flex-col gap-1 border-t border-primary pt-3">
              <dt className="order-2 font-ui text-sm leading-snug text-muted">{s.label}</dt>
              <dd className="order-1 font-ui text-heading tabular-nums">{s.value}</dd>
            </div>
          ))}
        </dl>
      </Section>

      <Section
        id="fast"
        title="Fast enough to run on every keystroke"
        lede={
          <p>
            Feanor lints a 65,000-line feature file in 0.2 seconds and formats it in 0.3. In an editor, it re-checks
            only the part you changed: about a millisecond per keystroke on a typical file.
          </p>
        }
      />

      <Section
        id="ci"
        title="Keep feature code tidy in CI"
        lede={
          <p>
            <code className="font-mono text-[14px]">feanor check</code> fails when a file isn&apos;t formatted or has
            errors, and with <code className="font-mono text-[14px]">--format github</code> the problems appear as
            annotations on the pull request. It follows includes and checks every <code className="font-mono text-[14px]">#ifdef</code>{" "}
            build.
          </p>
        }
      >
        <div className="grid gap-4 lg:grid-cols-2">
          <Code label="GitHub Actions">{CI}</Code>
          <figure className="flex flex-col gap-1.5">
            <figcaption className="font-ui text-xs text-muted">On the pull request</figcaption>
            <div className="rounded-lg border border-line bg-surface-raised p-4 font-ui text-sm">
              <p className="font-mono text-xs text-muted">features/common.fea, line 72</p>
              <p className="mt-2 flex gap-2">
                <span className="rounded bg-danger-surface px-1.5 font-mono text-[11px] leading-5 text-danger">error</span>
                <span>
                  <span className="font-mono text-[13px]">C003 undefined-class</span>: class @numbers_dflt is never
                  defined in the project
                </span>
              </p>
            </div>
          </figure>
        </div>
      </Section>

      <Section
        id="anywhere"
        title="Use it anywhere"
        lede={
          <p>
            Feanor is a Rust library first: the command-line tool, a WebAssembly build for the browser (this page runs
            on it), and Shift&apos;s feature editor are all built on the same crate. A language server for VS Code and
            Zed is next.
          </p>
        }
      >
        <div className="grid gap-4 lg:grid-cols-2">
          <Code label="Rust">{RUST}</Code>
          <Code label="JavaScript (WebAssembly)">{JS}</Code>
        </div>
      </Section>

      <footer className="flex flex-col gap-3 border-t border-line pt-6 font-ui text-xs text-muted">
        <p>
          Feanor is open source under the MIT or Apache 2.0 licence, made by{" "}
          <a href="https://shift.graphics" className="text-primary hover:underline">
            Shift
          </a>
          , the open-source font editor. Source on{" "}
          <a href={GITHUB} className="text-primary hover:underline">
            GitHub
          </a>
          .
        </p>
        <p>
          Example feature code: {examples.map((e) => e.credit).join(" ")}
        </p>
      </footer>
    </div>
  );
}
