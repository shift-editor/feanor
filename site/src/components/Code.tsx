import { type CodeLanguage, highlight } from "@/lib/highlight";

/** A highlighted code block with an optional caption. Rendered on the server. */
export async function Code({ code, lang, label }: { code: string; lang: CodeLanguage; label?: string }) {
  const html = await highlight(code, lang);
  return (
    <figure className="flex min-w-0 flex-col gap-1.5">
      {label && <figcaption className="font-ui text-xs text-muted">{label}</figcaption>}
      <div
        className="code-block overflow-x-auto rounded-lg border border-line bg-surface p-4"
        // Shiki's output: escaped code in coloured spans
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </figure>
  );
}
