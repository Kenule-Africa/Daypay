import Link from "next/link";
import { Logo } from "@/components/brand/logo";
import { SiteFooter } from "@/components/marketing/site-footer";

export function LegalFrame({ children }: { children: React.ReactNode }) {
  return (
    <>
      <header className="px-4 pt-5 sm:px-6">
        <div className="mx-auto flex max-w-reading items-center justify-between">
          <Logo />
          <Link href="/legal" className="rounded-md text-sm text-ink-500 transition-colors hover:text-ink-900">
            All legal documents
          </Link>
        </div>
      </header>
      <main id="main" className="mx-auto max-w-reading px-4 py-14 sm:px-6 sm:py-20">
        {children}
      </main>
      <SiteFooter />
    </>
  );
}
