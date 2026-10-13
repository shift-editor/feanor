/** One chapter of the page: a numbered label, a heading, a short paragraph, and a live demo. */
export function Section({
  id,
  eyebrow,
  title,
  children,
  lede,
}: {
  id: string;
  /** The small label above the heading, e.g. "02 · Linting". */
  eyebrow: string;
  title: string;
  lede: React.ReactNode;
  children?: React.ReactNode;
}) {
  return (
    <section id={id} className="flex flex-col gap-6 scroll-mt-16">
      <div className="flex max-w-[62ch] flex-col gap-3">
        <p className="eyebrow">{eyebrow}</p>
        <h2 className="font-release-heading text-heading text-balance">{title}</h2>
        <div className="font-copy text-copy text-secondary">{lede}</div>
      </div>
      {children}
    </section>
  );
}
