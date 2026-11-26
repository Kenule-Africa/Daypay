export function PageHeader({ title, description, eyebrow }: { title: string; description?: string; eyebrow?: string }) {
  return (
    <header className="mb-8 lg:mb-10">
      {eyebrow && <p className="text-xs font-semibold uppercase tracking-[0.18em] text-brand-600">{eyebrow}</p>}
      <h1 className="mt-2 font-display text-3xl font-semibold leading-tight text-ink-950 sm:text-4xl">{title}</h1>
      {description && <p className="mt-2 max-w-2xl text-[1.0625rem] leading-relaxed text-ink-500">{description}</p>}
    </header>
  );
}

/** A titled group of settings; tonal surface, no outline. */
export function Section({
  title,
  description,
  action,
  children,
  className,
}: {
  title: string;
  description?: React.ReactNode;
  action?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <section className={`rounded-feature bg-surface p-5 shadow-soft sm:p-7 ${className ?? ""}`}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="font-display text-lg font-semibold text-ink-900">{title}</h2>
          {description && <p className="mt-1 text-[0.9375rem] leading-relaxed text-ink-500">{description}</p>}
        </div>
        {action}
      </div>
      <div className="mt-5">{children}</div>
    </section>
  );
}
