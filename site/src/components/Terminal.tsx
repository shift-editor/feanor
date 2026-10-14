"use client";

import { useEffect, useRef, useState } from "react";
import { TrafficLights } from "./Window";

/** A command the tab strip runs in the terminal. */
export interface Command {
  id: string;
  label: string;
  /** A Nerd Font glyph (see src/fonts/SymbolsNerdFont-NOTICE.txt). */
  icon: string;
  command: string;
}

type RGB = [number, number, number];

const DIRECTORY = "~/my-font";
const BRANCH = "main";
const SEGMENTS: Record<"directory" | "branch" | "failed", RGB> = {
  directory: [14, 116, 144],
  branch: [71, 85, 105],
  failed: [220, 38, 38],
};
// Nerd Font glyphs: powerline separators and Octicons
const NF = { arrow: "", cap: "", folder: "", branch: "", cross: "" };

// the page's palette as a terminal theme; Feanor uses red, green, yellow, blue and cyan
const THEME = {
  background: "#ffffff",
  foreground: "#1f1f1f",
  cursor: "#1f1f1f",
  cursorAccent: "#ffffff",
  selectionBackground: "#cfe8fc",
  black: "#1f1f1f",
  red: "#dc2626",
  green: "#16a34a",
  yellow: "#c2410c",
  blue: "#64748b",
  magenta: "#9333ea",
  cyan: "#0e7490",
  white: "#e5e5e5",
  brightBlack: "#6b7280",
  brightRed: "#dc2626",
  brightGreen: "#16a34a",
  brightYellow: "#c2410c",
  brightBlue: "#64748b",
  brightMagenta: "#9333ea",
  brightCyan: "#0e7490",
  brightWhite: "#ffffff",
};

const fg = ([r, g, b]: RGB) => `\x1b[38;2;${r};${g};${b}m`;
const bg = ([r, g, b]: RGB) => `\x1b[48;2;${r};${g};${b}m`;
const WHITE_BOLD = "\x1b[1;38;2;255;255;255m";
const RESET = "\x1b[0m";
const DIM = "\x1b[2m";

/** An oh-my-posh style powerline prompt; after a failed command, a red segment with its exit status. */
function prompt(status: number): string {
  const segments: { color: RGB; text: string }[] = [
    { color: SEGMENTS.directory, text: `${NF.folder} ${DIRECTORY}` },
    { color: SEGMENTS.branch, text: `${NF.branch} ${BRANCH}` },
    ...(status === 0 ? [] : [{ color: SEGMENTS.failed, text: `${NF.cross} ${status}` }]),
  ];
  let out = "";
  segments.forEach((s, i) => {
    const next = segments[i + 1];
    out += `${bg(s.color)}${WHITE_BOLD} ${s.text} ${RESET}`;
    out += `${fg(s.color)}${next ? bg(next.color) : ""}${NF.arrow}${RESET}`;
  });
  return `${out} `;
}

/** Split a command line into words, honouring single and double quotes. */
function words(line: string): string[] {
  const out: string[] = [];
  for (const m of line.matchAll(/"([^"]*)"|'([^']*)'|(\S+)/g)) out.push(m[1] ?? m[2] ?? m[3]);
  return out;
}

const stripAnsi = (s: string) => s.replace(/\x1b\[[0-9;]*m/g, "");

const HELP = `Commands:
  feanor ...      the Feanor command-line tool (try feanor --help)
  ls              list the files
  cat FILE        show a file
  clear           clear the screen
  help            this list

The files are in memory: changes made by feanor fmt --write or
feanor lint --fix stay until you reload the page.
`;

/**
 * The real Feanor command-line tool, compiled to WebAssembly (WASI), in a
 * terminal emulator (ghostty-web, libghostty compiled to WebAssembly). Both load
 * when the terminal scrolls into view. The files live in memory.
 */
export function Terminal({ files, commands }: { files: Record<string, string>; commands: Command[] }) {
  const screen = useRef<HTMLDivElement>(null);
  const run = useRef<((line: string) => Promise<void>) | null>(null);
  const [state, setState] = useState<"waiting" | "loading" | "ready" | "failed">("waiting");
  const [transcript, setTranscript] = useState("");

  useEffect(() => {
    const element = screen.current;
    if (!element) return;
    let disposed = false;
    let cleanup = () => {};

    async function start(element: HTMLDivElement) {
      setState("loading");
      const [ghostty, shim, cli] = await Promise.all([
        import("ghostty-web"),
        import("@bjorn3/browser_wasi_shim"),
        WebAssembly.compileStreaming(fetch("/feanor-cli.wasm")),
      ]);
      await ghostty.init();
      const css = getComputedStyle(document.documentElement);
      const mono = css.getPropertyValue("--font-paper-mono").trim();
      const nerd = css.getPropertyValue("--font-nerd").trim();
      // load the fonts before the terminal measures its cells; only the first family of each list, since
      // next/font's "Fallback" faces point at local fonts that may be missing, which rejects the whole load
      const first = (families: string) => families.split(",")[0].trim();
      await Promise.allSettled([
        document.fonts.load(`13px ${first(mono)}`),
        document.fonts.load(`13px ${first(nerd)}`, NF.arrow),
      ]);
      if (disposed) return;

      const term = new ghostty.Terminal({
        fontSize: 13,
        fontFamily: `${mono}, ${nerd}`,
        theme: THEME,
        convertEol: true,
        scrollback: 2000,
        cursorStyle: "block",
      });
      term.open(element);
      const fit = new ghostty.FitAddon();
      term.loadAddon(fit);
      fit.fit();
      fit.observeResize();

      const encoder = new TextEncoder();
      const decoder = new TextDecoder();
      const dir = new shim.PreopenDirectory(
        ".",
        new Map(Object.entries(files).map(([name, text]) => [name, new shim.File(encoder.encode(text))])),
      );
      const fileNames = () => [...dir.dir.contents.keys()].sort();

      async function feanor(args: string[]): Promise<{ status: number; output: string }> {
        let output = "";
        const write = (buffer: Uint8Array) => {
          const text = decoder.decode(buffer, { stream: true });
          output += text;
          term.write(text);
        };
        const wasi = new shim.WASI(
          ["feanor", ...args],
          ["FORCE_COLOR=1"],
          [new shim.OpenFile(new shim.File([])), new shim.ConsoleStdout(write), new shim.ConsoleStdout(write), dir],
        );
        const instance = await WebAssembly.instantiate(cli, { wasi_snapshot_preview1: wasi.wasiImport });
        try {
          const status = wasi.start(instance as { exports: { memory: WebAssembly.Memory; _start: () => unknown } });
          return { status, output };
        } catch (e) {
          const message = `feanor: ${e instanceof Error ? e.message : String(e)}\n`;
          term.write(message);
          return { status: 1, output: output + message };
        }
      }

      async function execute(line: string): Promise<{ status: number; output: string }> {
        const [name, ...args] = words(line);
        const say = (text: string, status = 0) => {
          term.write(text);
          return { status, output: text };
        };
        switch (name) {
          case undefined:
            return { status: 0, output: "" };
          case "feanor":
            return feanor(args);
          case "ls":
            return say(fileNames().join("  ") + "\n");
          case "cat": {
            if (!args.length) return say("cat: missing file name\n", 1);
            let text = "";
            for (const a of args) {
              const f = dir.dir.contents.get(a);
              if (!(f instanceof shim.File)) return say(`${text}cat: ${a}: No such file or directory\n`, 1);
              text += decoder.decode(f.data);
            }
            return say(text.endsWith("\n") ? text : text + "\n");
          }
          case "clear":
            term.clear();
            return { status: 0, output: "" };
          case "help":
            return say(HELP);
          case "echo":
            return say(args.join(" ") + "\n");
          default:
            return say(`zsh: command not found: ${name}\n`, 127);
        }
      }

      let line = "";
      let status = 0;
      let busy = false;
      const history: string[] = [];
      let back = 0;

      const replaceLine = (text: string) => {
        term.write("\b \b".repeat(line.length) + text);
        line = text;
      };

      async function submit(command: string) {
        busy = true;
        term.write("\n");
        if (command.trim()) history.push(command);
        back = 0;
        const result = await execute(command);
        status = result.status;
        setTranscript(`${command}\n${stripAnsi(result.output)}\nexit status ${status}`);
        term.write(prompt(status));
        line = "";
        busy = false;
      }

      run.current = async (command: string) => {
        if (busy) return;
        replaceLine(command);
        await submit(command);
        term.focus();
      };

      const onData = term.onData(async (data: string) => {
        if (busy) return;
        if (data === "\r") return submit(line);
        if (data === "\x7f" || data === "\b") {
          if (line) replaceLine(line.slice(0, -1));
          return;
        }
        if (data === "\x03") {
          term.write("^C\n" + prompt(status));
          line = "";
          return;
        }
        if (data === "\x0c") {
          term.clear();
          return;
        }
        if (data === "\x1b[A" || data === "\x1b[B") {
          back = Math.max(0, Math.min(history.length, back + (data === "\x1b[A" ? 1 : -1)));
          replaceLine(back ? history[history.length - back] : "");
          return;
        }
        if (data === "\t") {
          const partial = line.split(" ").pop() ?? "";
          const pool = line.includes(" ") ? fileNames() : ["feanor", "ls", "cat", "clear", "help", "echo"];
          const matches = pool.filter((n) => n.startsWith(partial));
          if (matches.length === 1) replaceLine(line + matches[0].slice(partial.length) + " ");
          return;
        }
        if (data.startsWith("\x1b")) return;
        // typed or pasted text; a pasted line break runs the command
        const [first, ...rest] = data.replace(/\r\n?/g, "\n").split("\n");
        const printable = first.replace(/[\x00-\x1f]/g, "");
        term.write(printable);
        line += printable;
        if (rest.length) await submit(line);
      });

      term.write(
        `${DIM}The Feanor command-line tool, compiled to WebAssembly and running in your browser.\n` +
          `Files: ${fileNames().join(", ")} (in memory). Try a command above, or type help.${RESET}\n\n`,
      );
      term.write(prompt(0));
      setState("ready");
      cleanup = () => {
        onData.dispose();
        term.dispose();
      };
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          observer.disconnect();
          start(element).catch((e) => {
            console.error(e);
            setState("failed");
          });
        }
      },
      { rootMargin: "200px" },
    );
    observer.observe(element);
    return () => {
      disposed = true;
      observer.disconnect();
      cleanup();
    };
  }, [files]);

  return (
    <section
      aria-label="Terminal"
      data-state={state}
      className="flex flex-col overflow-hidden rounded-[10px] border border-black/15 bg-white font-ui shadow-[0_12px_40px_-12px_rgba(0,0,0,0.3),0_4px_12px_-6px_rgba(0,0,0,0.12)]"
    >
      <header className="grid h-9 shrink-0 grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center bg-[#f6f6f6] pl-3.5 select-none">
        <TrafficLights />
        <p className="truncate px-4 text-[12px] whitespace-nowrap">
          <span className="font-medium">my-font</span>
          <span className="text-muted"> — zsh</span>
        </p>
        <span />
      </header>
      <nav aria-label="Commands" className="flex border-y border-line-subtle bg-[#ececec] text-[12px] select-none">
        {commands.map((c) => (
          <button
            key={c.id}
            type="button"
            title={c.command}
            disabled={state !== "ready"}
            onClick={() => run.current?.(c.command)}
            className="flex flex-1 cursor-pointer items-center justify-center gap-1.5 truncate border-l border-line-subtle px-3 py-1.5 text-muted outline-none first:border-l-0 hover:bg-black/[0.04] hover:text-primary focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:ring-inset disabled:cursor-default"
          >
            <span aria-hidden style={{ fontFamily: "var(--font-nerd)" }}>
              {c.icon}
            </span>
            {c.label}
          </button>
        ))}
      </nav>
      <div className="relative h-[30rem] bg-white px-3 py-2">
        <div ref={screen} className="h-full w-full" />
        {state !== "ready" && (
          <p className="absolute inset-0 grid place-items-center font-copy text-sm text-muted">
            {state === "failed" ? "The terminal couldn't start in this browser." : "Starting the terminal…"}
          </p>
        )}
      </div>
      <p aria-live="polite" className="sr-only" data-transcript>
        {transcript}
      </p>
    </section>
  );
}
