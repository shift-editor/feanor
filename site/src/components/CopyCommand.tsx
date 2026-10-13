"use client";

import { Check } from "lucide-react";
import { useState } from "react";

/** Two overlapping squares, drawn in the text colour. */
function CopyIcon() {
  return (
    <svg aria-hidden viewBox="0 0 24 24" fill="none" className="size-4" stroke="currentColor" strokeWidth={1.5}>
      <path
        d="M19.4 20H9.6C9.26863 20 9 19.7314 9 19.4V9.6C9 9.26863 9.26863 9 9.6 9H19.4C19.7314 9 20 9.26863 20 9.6V19.4C20 19.7314 19.7314 20 19.4 20Z"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M15 9V4.6C15 4.26863 14.7314 4 14.4 4H4.6C4.26863 4 4 4.26863 4 4.6V14.4C4 14.7314 4.26863 15 4.6 15H9"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/** A shell command with a copy button. */
export function CopyCommand({ command }: { command: string }) {
  const [copied, setCopied] = useState(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // clipboard refused (permissions, insecure context): the text is still selectable
    }
  }
  return (
    <div className="flex items-center gap-3 card px-3 py-2 font-mono text-sm">
      <span className="select-none text-placeholder">$</span>
      <code className="select-all">{command}</code>
      <button
        type="button"
        onClick={copy}
        aria-label={copied ? "Copied" : "Copy command"}
        title={copied ? "Copied" : "Copy"}
        className="ml-1 grid size-6 cursor-pointer place-items-center rounded text-muted transition-colors hover:bg-hover/50 hover:text-primary"
      >
        {copied ? <Check aria-hidden className="size-4" strokeWidth={1.75} /> : <CopyIcon />}
      </button>
    </div>
  );
}
