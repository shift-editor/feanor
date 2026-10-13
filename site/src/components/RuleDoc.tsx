import ReactMarkdown from "react-markdown";
import { highlight } from "@/lib/highlight";

/** A ```fea fence in rule documentation, highlighted on the server. */
async function Fence({ code }: { code: string }) {
  const html = await highlight(code.replace(/\n$/, ""), "fea");
  return <div className="code-block my-2 overflow-x-auto rounded-md bg-surface-raised p-3" dangerouslySetInnerHTML={{ __html: html }} />;
}

/** A rule's Markdown documentation, with its FEA examples highlighted. */
export function RuleDoc({ markdown }: { markdown: string }) {
  return (
    <div className="prose-rule">
      <ReactMarkdown
        components={{
          // fenced blocks arrive as <pre><code>; render the whole block ourselves
          pre: ({ node }) => {
            const code = node?.children[0];
            if (code?.type !== "element") return null;
            const text = code.children.map((c) => (c.type === "text" ? c.value : "")).join("");
            return <Fence code={text} />;
          },
        }}
      >
        {markdown}
      </ReactMarkdown>
    </div>
  );
}
