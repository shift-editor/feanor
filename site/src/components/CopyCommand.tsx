"use client";

import { useState } from "react";

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
    <div className="flex items-center gap-3 rounded-lg border border-line bg-surface px-3 py-2 font-mono text-sm">
      <span className="select-none text-placeholder">$</span>
      <code className="select-all">{command}</code>
      <button type="button" onClick={copy} className="ml-1 font-ui text-xs text-muted hover:text-primary">
        {copied ? "Copied" : "Copy"}
      </button>
    </div>
  );
}
