import ReactMarkdown from "react-markdown";
import { Code } from "@/components/Code";
import { CopyCommand } from "@/components/CopyCommand";
import { type Example, FormatDemo } from "@/components/FormatDemo";
import { LintDemo } from "@/components/LintDemo";
import { Playground } from "@/components/Playground";
import { Section } from "@/components/Section";
import { SpeedCharts } from "@/components/SpeedCharts";
import { credit, example, glyphList, ruleDocs } from "@/lib/content";

const VERSION = "0.1.0";
const GITHUB = "https://github.com/shift-editor/feanor";

const NAV = [
  { label: "Rules", href: "#rules" },
  { label: "CI", href: "#ci" },
  { label: "Docs", href: "https://docs.rs/feanor" },
];

const STATS = [
  { value: "2,762", label: "feature files from the upstream sources of every Google Fonts family" },
  { value: "0", label: "files that compile differently after formatting, checked with fea-rs and fontTools" },
  { value: "2,749 / 2,749", label: "planted mistakes found by the linter, at the right place" },
];

const CI = `name: Feature code
on: [push, pull_request]

jobs:
  feanor:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - run: cargo install feanor-cli --locked
      - run: feanor check features.fea --format github`;

const CONFIG = `# feanor.toml
line-width = 100

[lint]
# check glyph names against the font
glyphs = "sources/MyFont-Regular.ufo"
# unused classes are fine in this project
ignore = ["C004"]`;

const RUST = `use feanor::{
    format, lint, FormatOptions, GlyphNames, LintOptions,
};

let glyphs: GlyphNames = names.into_iter().collect();
let options = LintOptions::default().with_glyphs(&glyphs);

for d in lint(&source, &options) {
    println!("{}: {}", d.rule.code(), d.message);
}

let tidy = format(&source, &FormatOptions::default());`;

const JS = `import init, { Document, format } from "@shift-editor/feanor";

await init();

// As the user types, only the edited part is checked again
const doc = new Document(source, { glyphs });
doc.edit(from, to, text);
const diagnostics = doc.diagnostics();

const tidy = format(source, { lineWidth: 100 });`;

function GithubIcon() {
  return (
    <svg viewBox="0 0 16 16" aria-hidden className="size-5" fill="currentColor">
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" />
    </svg>
  );
}

export default async function Home() {
  const examples: Example[] = [
    { id: "inter", label: "Inter: calt.fea", file: "inter-calt.fea" },
    { id: "recursive", label: "Recursive: common.fea", file: "recursive-common.fea" },
    { id: "nastaliq", label: "Noto Nastaliq Urdu: classes", file: "noto-nastaliq-classes.fea" },
  ].map((e) => ({ ...e, text: example(e.file), credit: credit(e.file) }));
  const glyphs = glyphList("playground-glyphs.txt");
  const rules = ruleDocs();

  return (
    <div className="mx-auto flex max-w-[68rem] flex-col gap-24 px-4 pb-20 sm:px-6">
      <header className="font-ui">
        <nav
          aria-label="Main navigation"
          className="grid grid-cols-[1fr_auto] items-center gap-x-6 pt-8 text-sm sm:grid-cols-[1fr_auto_1fr] [&_a]:focus-visible:outline-2 [&_a]:focus-visible:outline-offset-4 [&_a]:focus-visible:outline-accent"
        >
          <a href="#" className="justify-self-start font-release-heading text-xl font-bold tracking-[-0.06em]">
            Feanor.
          </a>
          <div className="hidden items-center gap-8 sm:flex">
            {NAV.map((l) => (
              <a key={l.label} href={l.href} className="transition-colors hover:text-accent">
                {l.label}
              </a>
            ))}
          </div>
          <div className="flex items-center gap-4 justify-self-end">
            <a href={GITHUB} aria-label="Feanor on GitHub" className="transition-opacity hover:opacity-60">
              <GithubIcon />
            </a>
            <a href="#install" className="button-accent">
              Install
            </a>
          </div>
        </nav>
      </header>

      <section className="flex flex-col gap-8">
        <div className="flex max-w-[44rem] flex-col gap-5">
          <h1 className="font-release-heading text-[2.5rem] leading-[1.05] font-bold tracking-[-0.06em] text-balance sm:text-display">
            Format and lint OpenType feature files
          </h1>
          <p className="max-w-[56ch] font-copy text-[1.0625rem] leading-relaxed text-secondary">
            Feanor is a fast formatter and linter for <code className="font-mono text-[15px]">.fea</code> code,
            written in Rust on fea-rs. Formatting only touches whitespace, so it never changes the compiled font.
            The linter finds misspelled glyphs, mismatched classes, and invisible characters, with fixes.
          </p>
          <div id="install" className="flex scroll-mt-8 flex-wrap items-center gap-3">
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
              <dt className="order-2 font-copy text-sm leading-snug text-muted">{s.label}</dt>
              <dd className="order-1 font-release-heading text-heading">{s.value}</dd>
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
            only the part you changed, so even the largest file re-checks in a couple of milliseconds per
            keystroke.
          </p>
        }
      >
        <SpeedCharts />
      </Section>

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
        <div className="grid grid-cols-[minmax(0,1fr)] gap-4 xl:grid-cols-2">
          <Code label=".github/workflows/feanor.yml" lang="yaml" code={CI} />
          <div className="flex min-w-0 flex-col gap-4">
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
          <Code label="Settings, if you need any" lang="toml" code={CONFIG} />
          </div>
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
        <div className="grid max-w-[48rem] grid-cols-[minmax(0,1fr)] gap-6">
          <Code label="Rust" lang="rust" code={RUST} />
          <Code label="JavaScript (WebAssembly)" lang="javascript" code={JS} />
        </div>
      </Section>

      <footer className="flex flex-col gap-3 border-t border-line pt-6 font-copy text-xs text-muted">
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
