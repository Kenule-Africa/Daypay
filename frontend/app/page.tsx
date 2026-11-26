import { cookies } from "next/headers";
import { Hero } from "@/components/marketing/hero";
import {
  Availability,
  AutoSettlement,
  ClosingCall,
  MoveMoney,
  PayAnyone,
  Purpose,
  Security,
  Wallets,
} from "@/components/marketing/sections";
import { SiteFooter } from "@/components/marketing/site-footer";
import { SiteHeader } from "@/components/marketing/site-header";
import { SESSION_COOKIE } from "@/lib/server/session";

export default async function Home() {
  // Only selects which call to action to show; /app validates the session itself.
  const signedIn = (await cookies()).has(SESSION_COOKIE);
  return (
    <>
      <a
        href="#main"
        className="sr-only z-50 rounded-control bg-ink-900 px-4 py-2 text-paper focus:not-sr-only focus:fixed focus:left-4 focus:top-4"
      >
        Skip to content
      </a>
      <SiteHeader signedIn={signedIn} />
      <main id="main">
        <Hero signedIn={signedIn} />
        <Purpose />
        <Wallets />
        <MoveMoney />
        <PayAnyone />
        <AutoSettlement />
        <Security />
        <Availability />
        <ClosingCall signedIn={signedIn} />
      </main>
      <SiteFooter />
    </>
  );
}
