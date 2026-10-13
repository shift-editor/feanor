import "server-only";
import pierreLight from "@pierre/theme/pierre-light";
import { codeToHtml, type ThemeRegistrationRaw } from "shiki";

// a VS Code theme, which Shiki takes as a raw registration
const theme = pierreLight as unknown as ThemeRegistrationRaw;

export type CodeLanguage = "yaml" | "rust" | "javascript" | "shellsession" | "toml";

/** Highlight code at build time, in the same theme as the diffs. */
export function highlight(code: string, lang: CodeLanguage): Promise<string> {
  return codeToHtml(code, { lang, theme });
}
