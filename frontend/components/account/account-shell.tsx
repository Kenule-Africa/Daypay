"use client";

import { LayoutDashboard, LogOut, ShieldCheck, SlidersHorizontal, UserRound } from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import * as React from "react";
import { useAccount } from "@/components/account/account-context";
import { Avatar } from "@/components/account/avatar";
import { useSignOut } from "@/components/account/use-sign-out";
import { LogoMark } from "@/components/brand/logo";
import { Notice } from "@/components/ui/notice";
import { Spinner } from "@/components/ui/spinner";
import { fullName } from "@/lib/account";
import { cn } from "@/lib/utils";

const navigation = [
  { href: "/app", label: "Overview", icon: LayoutDashboard },
  { href: "/app/profile", label: "Profile", icon: UserRound },
  { href: "/app/preferences", label: "Preferences", icon: SlidersHorizontal },
  { href: "/app/security", label: "Security", icon: ShieldCheck },
];

function isActive(pathname: string, href: string) {
  return href === "/app" ? pathname === "/app" : pathname.startsWith(href);
}

export function AccountShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const { profile } = useAccount();
  const { signOut, pending, error } = useSignOut();

  // A page restored from the back/forward cache must be re-validated by the
  // server, so a signed-out or revoked session never shows private content.
  React.useEffect(() => {
    const onShow = (event: PageTransitionEvent) => {
      if (event.persisted) window.location.reload();
    };
    window.addEventListener("pageshow", onShow);
    return () => window.removeEventListener("pageshow", onShow);
  }, []);

  return (
    <div className="min-h-dvh lg:pl-[18.5rem]">
      {/* Desktop navigation rail */}
      <aside className="fixed inset-y-3 left-3 z-30 hidden w-[17rem] flex-col rounded-hero bg-ink-950 p-5 text-paper shadow-pop lg:flex">
        <Link href="/app" className="flex items-center gap-2.5 rounded-lg px-2 py-1 text-white">
          <LogoMark />
          <span className="font-display text-[1.15rem] font-semibold tracking-[-0.03em]">DEYPAY</span>
        </Link>
        <nav aria-label="Account" className="mt-10">
          <ul className="grid gap-1">
            {navigation.map(({ href, label, icon: Icon }) => {
              const active = isActive(pathname, href);
              return (
                <li key={href}>
                  <Link
                    href={href}
                    aria-current={active ? "page" : undefined}
                    className={cn(
                      "group relative flex items-center gap-3 rounded-control px-3 py-2.5 text-[0.9375rem] transition-colors duration-200",
                      active ? "bg-white/[0.08] text-white" : "text-ink-300 hover:bg-white/[0.04] hover:text-white",
                    )}
                  >
                    <span
                      aria-hidden
                      className={cn(
                        "absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-full bg-brand-400 transition-opacity duration-300",
                        active ? "opacity-100" : "opacity-0",
                      )}
                    />
                    <Icon aria-hidden className={cn("size-[1.15rem]", active ? "text-brand-300" : "text-ink-400 group-hover:text-ink-200")} />
                    {label}
                  </Link>
                </li>
              );
            })}
          </ul>
        </nav>
        <div className="mt-auto">
          {error && (
            <Notice tone="error" className="mb-3 text-[0.8125rem]">
              {error}
            </Notice>
          )}
          <div className="flex items-center gap-3 rounded-panel bg-white/[0.05] p-3">
            <Avatar profile={profile} className="size-10 text-sm" />
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-white">{fullName(profile)}</p>
              <p className="truncate text-xs text-ink-400">{`@${profile.username}`}</p>
            </div>
            <button
              type="button"
              onClick={signOut}
              disabled={pending}
              aria-label="Sign out"
              className="inline-flex size-9 items-center justify-center rounded-[0.6rem] text-ink-300 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-60"
            >
              {pending ? <Spinner /> : <LogOut aria-hidden className="size-[1.1rem]" />}
            </button>
          </div>
        </div>
      </aside>

      {/* Mobile top bar */}
      <header className="sticky top-0 z-30 flex items-center justify-between bg-paper/85 px-4 pb-3 pt-[max(0.75rem,env(safe-area-inset-top))] backdrop-blur-xl lg:hidden">
        <Link href="/app" className="flex items-center gap-2 rounded-lg" aria-label="DEYPAY overview">
          <LogoMark className="size-7" />
          <span className="font-display text-base font-semibold tracking-[-0.03em]">DEYPAY</span>
        </Link>
        <Link href="/app/profile" aria-label="Your profile" className="rounded-full">
          <Avatar profile={profile} className="size-9 text-xs" />
        </Link>
      </header>

      <main id="main" className="mx-auto w-full max-w-app px-4 pb-32 pt-4 sm:px-6 lg:px-10 lg:pb-16 lg:pt-12">
        {children}
      </main>

      {/* Mobile tab bar */}
      <nav
        aria-label="Account"
        className="fixed inset-x-3 bottom-[max(0.75rem,env(safe-area-inset-bottom))] z-30 rounded-[1.4rem] bg-ink-950/95 p-1.5 shadow-pop backdrop-blur-xl lg:hidden"
      >
        <ul className="grid grid-cols-4">
          {navigation.map(({ href, label, icon: Icon }) => {
            const active = isActive(pathname, href);
            return (
              <li key={href}>
                <Link
                  href={href}
                  aria-current={active ? "page" : undefined}
                  className={cn(
                    "flex flex-col items-center gap-1 rounded-[1.05rem] py-2 text-[0.6875rem] font-medium transition-colors duration-200",
                    active ? "bg-white/10 text-white" : "text-ink-400 hover:text-ink-200",
                  )}
                >
                  <Icon aria-hidden className={cn("size-5", active && "text-brand-300")} />
                  {label}
                </Link>
              </li>
            );
          })}
        </ul>
      </nav>
    </div>
  );
}
