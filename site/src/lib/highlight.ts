import "server-only";
import pierreLight from "@pierre/theme/pierre-light";
import { createHighlighter, type ThemeRegistrationRaw } from "shiki";
import { feaGrammar } from "./fea-grammar";

// a VS Code theme, which Shiki takes as a raw registration
const theme = pierreLight as unknown as ThemeRegistrationRaw;
const LANGUAGES = ["yaml", "rust", "javascript", "shellsession", "toml"] as const;

export type CodeLanguage = (typeof LANGUAGES)[number] | "fea";

let highlighter: ReturnType<typeof createHighlighter> | null = null;

/** One highlighter for the whole build, with FEA added to Shiki's languages. */
function load(): ReturnType<typeof createHighlighter> {
  if (!highlighter) highlighter = createHighlighter({ themes: [theme], langs: [...LANGUAGES, feaGrammar] });
  return highlighter;
}

/** Highlight code at build time, in the same theme as the diffs. */
export async function highlight(code: string, lang: CodeLanguage): Promise<string> {
  const h = await load();
  return h.codeToHtml(code, { lang, theme: theme.name ?? "pierre-light" });
}
