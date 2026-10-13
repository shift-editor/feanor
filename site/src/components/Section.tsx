/** One block of the page: a heading, a short paragraph, and a live demo. */
export function Section({
  id,
  title,
  children,
  lede,
}: {
  id: string;
  title: string;
  lede: React.ReactNode;
  children?: React.ReactNode;
}) {
  return (
    <section id={id} className="flex flex-col gap-5 scroll-mt-16">
      <div className="flex max-w-[62ch] flex-col gap-2">
        <h2 className="font-release-heading text-heading text-balance">{title}</h2>
        <div className="font-copy text-copy text-secondary">{lede}</div>
      </div>
      {children}
    </section>
  );
}
