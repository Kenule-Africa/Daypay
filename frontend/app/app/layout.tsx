import { RotateCw, ServerCrash } from "lucide-react";
import type { Metadata } from "next";
import Link from "next/link";
import { redirect } from "next/navigation";
import { AccountProvider } from "@/components/account/account-context";
import { AccountShell } from "@/components/account/account-shell";
import { Logo } from "@/components/brand/logo";
import { Button } from "@/components/ui/button";
import { loadCustomer } from "@/lib/server/session";

export const metadata: Metadata = {
  title: { default: "Your account", template: "%s · DEYPAY" },
  robots: { index: false, follow: false },
};

/**
 * Server-side guard for the whole account area. Every request re-validates the
 * session cookie with the API before anything private renders.
 */
export default async function AccountLayout({ children }: { children: React.ReactNode }) {
  const customer = await loadCustomer();
  if (customer.state === "signed_out") redirect("/login?next=/app");
  if (customer.state === "expired") redirect("/login?reason=expired&next=/app");
  if (customer.state === "unavailable") return <AccountUnavailable />;
  return (
    <AccountProvider initial={customer.profile}>
      <AccountShell>{children}</AccountShell>
    </AccountProvider>
  );
}

function AccountUnavailable() {
  return (
    <main className="mx-auto flex min-h-dvh max-w-form flex-col px-5 py-6">
      <Logo />
      <div className="my-auto animate-rise py-16">
        <span className="inline-flex size-14 items-center justify-center rounded-feature bg-caution-soft text-caution">
          <ServerCrash aria-hidden className="size-7" />
        </span>
        <h1 className="mt-7 font-display text-3xl font-semibold text-ink-950">We can&apos;t load your account right now</h1>
        <p className="mt-3 text-[1.0625rem] leading-relaxed text-ink-600">
          DEYPAY&apos;s account service isn&apos;t responding. Your account is safe; please try again in a moment.
        </p>
        <div className="mt-8 grid gap-3">
          <Button asChild size="lg">
            <Link href="/app" prefetch={false}>
              <RotateCw aria-hidden /> Try again
            </Link>
          </Button>
          <Button asChild size="lg" variant="quiet">
            <Link href="/">Back to home</Link>
          </Button>
        </div>
      </div>
    </main>
  );
}
