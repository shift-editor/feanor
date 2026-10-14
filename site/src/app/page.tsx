import { Code } from "@/components/Code";
import { CopyCommand } from "@/components/CopyCommand";
import { type Example, FormatDemo } from "@/components/FormatDemo";
import { LintDemo } from "@/components/LintDemo";
import { Playground } from "@/components/Playground";
import { PullRequest } from "@/components/PullRequest";
import { RuleList, RuleSummary } from "@/components/RuleList";
import { Section } from "@/components/Section";
import { type Scene, ShapingScenes } from "@/components/ShapingScene";
import { SpeedCharts } from "@/components/SpeedCharts";
import { credit, example, glyphList, ruleDocs } from "@/lib/content";
import duplicatePair from "@/scenes/duplicate-pair.json";
import unreachableImatra from "@/scenes/unreachable-imatra.json";
import unreachableRule from "@/scenes/unreachable-rule.json";

const GITHUB = "https://github.com/shift-editor/feanor";

const NAV = [
  { label: "Rules", href: "#rules" },
  { label: "CI", href: "#ci" },
  { label: "Docs", href: "https://docs.rs/feanor" },
];

/** Fonts whose feature code or outlines appear on the page: the code examples, and Inter Bold and Martel Bold in the scenes. */
const CREDITS = [
  { name: "Caveat", href: "https://github.com/googlefonts/caveat" },
  { name: "Zilla Slab", href: "https://github.com/mozilla/zilla-slab" },
  { name: "Recursive", href: "https://github.com/arrowtype/recursive" },
  { name: "Inter", href: "https://github.com/rsms/inter" },
  { name: "Martel", href: "https://github.com/typeoff/martel" },
];

const NEXT = [
  { label: "Install", title: "cargo install feanor-cli", href: "#install" },
  { label: "Read", title: "The library docs", href: "https://docs.rs/feanor" },
  { label: "Source", title: "Feanor on GitHub", href: GITHUB },
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

/** Scene code without the default `languagesystem` line, unless the scene is about it. */
const withoutHeader = (fea: string) =>
  fea.split("languagesystem").length > 2 ? fea : fea.replace("languagesystem DFLT dflt;\n", "");

// how often each mistake turns up: linting the feature code in the sources of every Google Fonts family
const SCENES = [
  {
    id: "unreachable-imatra",
    title: "An i-matra that never reaches",
    data: unreachableImatra,
    file: "features.fea",
    found: "From a font as it ships. Found in 4 Google Fonts families.",
  },
  {
    id: "duplicate-pair",
    title: "Kerning written twice",
    data: duplicatePair,
    file: "kern.fea",
    pair: 0,
    found: "Found in 4 Google Fonts families.",
  },
  {
    id: "unreachable-rule",
    title: "An exception that comes too late",
    data: unreachableRule,
    file: "calt.fea",
    found: "Found in 3 Google Fonts families.",
  },
];

export default async function Home() {
  const examples: Example[] = [
    { id: "hand-edited", label: "Hand-edited over the years", file: "caveat-calt.fea" },
    { id: "table", label: "A table, half aligned", file: "zilla-figures.fea" },
    { id: "tidy", label: "Already tidy", file: "recursive-common.fea" },
  ].map((e) => ({ ...e, text: example(e.file), credit: credit(e.file) }));
  const glyphs = glyphList("playground-glyphs.txt");
  const rules = ruleDocs();
  const scenes: Scene[] = SCENES.map(({ file, ...s }) => ({
    ...s,
    code: {
      written: <Code label={file} lang="fea" code={withoutHeader(s.data.written.fea)} />,
      meant: <Code label={`${file}, fixed`} lang="fea" code={withoutHeader(s.data.meant.fea)} />,
    },
  }));

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

      <section className="flex flex-col gap-10">
        <div className="flex max-w-[46rem] flex-col gap-5">
          <p className="eyebrow">A formatter and linter for OpenType feature files</p>
          <h1 className="font-release-heading text-[2.5rem] leading-[1.05] font-bold tracking-[-0.06em] text-balance sm:text-display">
            Feature code finally gets the tools the rest of the font has
          </h1>
          <p className="max-w-[58ch] font-copy text-[1.0625rem] leading-relaxed text-secondary">
            The <code className="font-mono text-[15px]">.fea</code> code behind a font&apos;s kerning, ligatures and
            alternates is written by hand, copied between families, and easy to get subtly wrong. Feanor formats it
            without ever changing the compiled font, and points out mistakes as you type, with fixes.
          </p>
          <div id="install" className="flex scroll-mt-8 flex-wrap items-center gap-x-7 gap-y-4 pt-1">
            <CopyCommand command="cargo install feanor-cli" />          </div>
        </div>
        <Playground initial={example("playground.fea")} glyphs={glyphs} />
      </section>

      <Section
        id="formatting"
        title="Safe to run on any font"
        lede={
          <p>
            Feanor changes spacing, indentation, line breaks and blank lines, and nothing else. Classes written on
            one line are packed to the line width, while classes you broke into rows keep your rows. Columns you
            aligned by hand stay aligned, and the file keeps its own indent and line endings. Try it on feature code
            from real fonts.
          </p>
        }
      >
        <FormatDemo examples={examples} />
        <div className="mt-10 flex flex-col gap-6">
          <div className="flex max-w-[62ch] flex-col gap-2">
            <h3 className="font-release-heading text-subheading">Checked on all of Google Fonts</h3>
            <p className="font-copy text-copy text-secondary">
              Every release formats the feature code from the source repository of every Google Fonts family, then
              compiles each file before and after with two independent compilers. The layout tables must come out
              identical.
            </p>
          </div>
          <dl className="grid gap-x-8 gap-y-10 pr-[9px] sm:grid-cols-3">
            {STATS.map((s) => (
              <div key={s.label} className="card flex flex-col gap-2 p-5">
                <dt className="order-2 font-copy text-sm leading-snug text-secondary">{s.label}</dt>
                <dd className="order-1 font-release-heading text-heading">{s.value}</dd>
              </div>
            ))}
          </dl>
        </div>
      </Section>

      <Section
        id="linting"
        title="Mistakes, caught where they are"
        lede={
          <p>
            Misspelled glyphs, classes that don&apos;t line up, invisible characters that arrived by copy-paste. Every
            diagnostic points at the exact text, explains itself, and often comes with a fix. Safe fixes can&apos;t
            change what the font does. Unsafe ones, like renaming a glyph to the closest match, wait for you.
          </p>
        }
      >
        <LintDemo
          initial={example("lint.fea")}
          glyphs={glyphs}
          explanations={Object.fromEntries(rules.map((r) => [r.code, <RuleSummary key={r.code} rule={r} />]))}
        />
        <div id="consequences" className="mt-10 flex scroll-mt-16 flex-col gap-6">
          <div className="flex max-w-[62ch] flex-col gap-2">
            <h3 className="font-release-heading text-subheading">What ships if you don&apos;t fix it</h3>
            <p className="font-copy text-copy text-secondary">
              Most of these mistakes compile without a word. Here is the same text set from the feature code as
              written and as meant, shaped by HarfBuzz, the engine in Chrome, Android and Figma.
            </p>
          </div>
          <ShapingScenes scenes={scenes} />
        </div>
        <div id="rules" className="mt-10 flex scroll-mt-16 flex-col gap-6">
          <div className="flex max-w-[62ch] flex-col gap-2">
            <h3 className="font-release-heading text-subheading">All {rules.length} rules</h3>
            <p className="font-copy text-copy text-secondary">
              Each rule has a stable code and a page of documentation. Choose rules by code, family or name in{" "}
              <code className="font-mono text-[14px]">feanor.toml</code>, on the command line, or with a{" "}
              <code className="font-mono text-[14px]"># noqa</code> comment.
            </p>
          </div>
        <RuleList rules={rules} />
        </div>
      </Section>

      <Section
        id="fast"
        title="Fast enough to run on every keystroke"
        lede={
          <p>
            Feanor lints or formats a 65,000-line feature file in a quarter of a second. In an editor, it re-checks
            only the part you changed, so even the largest file re-checks in under 10 milliseconds per keystroke.
          </p>
        }
      >
        <SpeedCharts />
      </Section>

      <Section
        id="ci"
        title="Tidy feature code on every pull request"
        lede={
          <p>
            <code className="font-mono text-[14px]">feanor check</code> fails when a file isn&apos;t formatted or has
            errors, and with <code className="font-mono text-[14px]">--format github</code> the problems appear as
            annotations on the pull request. It follows includes and checks every{" "}
            <code className="font-mono text-[14px]">#ifdef</code> build.
          </p>
        }
      >
        <PullRequest />
        <div className="mt-4 grid grid-cols-[minmax(0,1fr)] gap-6 xl:grid-cols-2">
          <Code label=".github/workflows/feanor.yml" lang="yaml" code={CI} />
          <Code label="Settings, if you need any" lang="toml" code={CONFIG} />
        </div>
      </Section>

      <Section
        id="anywhere"
        title="One library, everywhere feature code is edited"
        lede={
          <p>
            Feanor is a Rust library first: the command-line tool, a WebAssembly build for the browser (this page runs
            on it), and Shift&apos;s feature editor are all built on the same crate. A language server for VS Code and
            Zed is next.
          </p>
        }
      >
        <div className="grid grid-cols-[minmax(0,1fr)] gap-6 xl:grid-cols-2">
          <Code label="Rust" lang="rust" code={RUST} />
          <Code label="JavaScript (WebAssembly)" lang="javascript" code={JS} />
        </div>
      </Section>

      <nav aria-label="Next steps" className="grid gap-x-8 gap-y-6 pr-[9px] sm:grid-cols-3">
        {NEXT.map((n) => (
          <a key={n.label} href={n.href} className="card flex flex-col gap-2 px-6 py-5">
            <span className="eyebrow">{n.label} →</span>
            <span className="font-ui text-xl tracking-[-0.01em] text-primary">{n.title}</span>
          </a>
        ))}
      </nav>

      <footer className="flex flex-col gap-3 border-t border-line pt-6 font-copy text-xs text-muted">
        <p>
          Feanor is open source (MIT or Apache 2.0), made by{" "}
          <a href="https://shift.graphics" className="text-primary hover:underline">
            Shift
          </a>
          . Source on{" "}
          <a href={GITHUB} className="text-primary hover:underline">
            GitHub
          </a>
          .
        </p>
        {/* the OFL asks for the copyright notice and licence wherever the fonts' sources are shown */}
        <p>
          Examples from{" "}
          {CREDITS.map((c, i) => (
            <span key={c.name}>
              <a href={c.href} className="text-primary hover:underline">
                {c.name}
              </a>
              {i < CREDITS.length - 2 ? ", " : i === CREDITS.length - 2 ? " and " : ""}
            </span>
          ))}
          , © their authors, under the{" "}
          <a href="https://openfontlicense.org" className="text-primary hover:underline">
            SIL OFL 1.1
          </a>
          .
        </p>
      </footer>
    </div>
  );
}
