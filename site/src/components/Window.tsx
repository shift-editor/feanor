import { CircleX, Info, TriangleAlert } from "lucide-react";
import type { Diagnostic } from "@/lib/feanor";

/** The three macOS window buttons, for a title bar. */
export function TrafficLights() {
  return (
    <span aria-hidden className="flex shrink-0 items-center gap-2">
      <span className="size-3 rounded-full border border-black/10 bg-[#ff5f57]" />
      <span className="size-3 rounded-full border border-black/10 bg-[#febc2e]" />
      <span className="size-3 rounded-full border border-black/10 bg-[#28c840]" />
    </span>
  );
}

/**
 * A macOS app window laid out like Shift's feature code window: a title bar
 * with the traffic lights and a centred title, the content, and a status bar.
 */
export function AppWindow({
  title,
  detail,
  actions,
  status,
  children,
}: {
  title: string;
  /** Muted text after the title, e.g. the font's name. */
  detail?: string;
  /** Controls at the right of the title bar. */
  actions?: React.ReactNode;
  /** The status bar's contents; no status bar without it. */
  status?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section
      aria-label={title}
      className="flex flex-col overflow-hidden rounded-[10px] border border-black/15 bg-surface-raised font-ui shadow-[0_12px_40px_-12px_rgba(0,0,0,0.3),0_4px_12px_-6px_rgba(0,0,0,0.12)]"
    >
      <header className="grid h-9 shrink-0 grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center border-b border-line-subtle bg-[#f6f6f6] pl-3.5 select-none">
        <TrafficLights />
        <p className="truncate px-4 text-[12px] whitespace-nowrap">
          <span className="font-medium">{title}</span>
          {detail && <span className="text-muted"> — {detail}</span>}
        </p>
        <span className="flex items-center justify-end gap-1 pr-1.5">{actions}</span>
      </header>
      {children}
      {status && (
        <footer className="flex h-7 shrink-0 items-center gap-3.5 border-t border-line-subtle px-3 text-[11px] text-muted">
          {status}
        </footer>
      )}
    </section>
  );
}

/** Errors, warnings and hints as counts with icons, for a status bar. */
export function ProblemCount({ diagnostics }: { diagnostics: Diagnostic[] | null }) {
  if (!diagnostics) return <span>Loading Feanor…</span>;
  const count = (s: Diagnostic["severity"]) => diagnostics.filter((d) => d.severity === s).length;
  return (
    <span className="flex items-center gap-2.5 tabular-nums" aria-live="polite">
      <span className="flex items-center gap-1" aria-label={`${count("error")} errors`}>
        <CircleX aria-hidden className="size-3.5 text-[#dc2626]" />
        {count("error")}
      </span>
      <span className="flex items-center gap-1" aria-label={`${count("warning")} warnings`}>
        <TriangleAlert aria-hidden className="size-3.5 text-[#f97316]" />
        {count("warning")}
      </span>
      <span className="flex items-center gap-1" aria-label={`${count("hint")} hints`}>
        <Info aria-hidden className="size-3.5 text-[#9f9f9f]" />
        {count("hint")}
      </span>
    </span>
  );
}

/** A quiet text action for a title or status bar. */
export function BarButton({ children, ...props }: React.ComponentProps<"button">) {
  return (
    <button
      type="button"
      className="h-6 cursor-pointer rounded px-1.5 text-[11px] text-muted transition-colors hover:bg-hover/50 hover:text-primary focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:outline-none aria-pressed:bg-hover aria-pressed:text-primary"
      {...props}
    >
      {children}
    </button>
  );
}
