"use client";

import { ArrowRight, Menu, X } from "lucide-react";
import Link from "next/link";
import { Dialog } from "radix-ui";
import * as React from "react";
import { Logo } from "@/components/brand/logo";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const links = [
  { href: "#product", label: "Product" },
  { href: "#pay-anyone", label: "Pay Anyone" },
  { href: "#auto-settlement", label: "Auto-Settlement" },
  { href: "#security", label: "Security" },
  { href: "#availability", label: "Availability" },
];

export function SiteHeader({ signedIn }: { signedIn: boolean }) {
  const [scrolled, setScrolled] = React.useState(false);
  const [open, setOpen] = React.useState(false);

  React.useEffect(() => {
    const update = () => setScrolled(window.scrollY > 12);
    update();
    window.addEventListener("scroll", update, { passive: true });
    return () => window.removeEventListener("scroll", update);
  }, []);

  return (
    <header className="sticky top-0 z-40 px-3 pt-3 sm:px-5">
      <div
        className={cn(
          "mx-auto flex h-15 max-w-site items-center justify-between rounded-[1.1rem] pl-4 pr-2 transition-[background-color,box-shadow,backdrop-filter] duration-500 ease-out-expo sm:pl-5",
          scrolled ? "bg-paper/80 shadow-lift backdrop-blur-xl" : "bg-transparent",
        )}
      >
        <Logo />
        <nav aria-label="Primary" className="hidden lg:block">
          <ul className="flex items-center gap-1">
            {links.map(link => (
              <li key={link.href}>
                <a
                  href={link.href}
                  className="rounded-full px-3.5 py-2 text-[0.9rem] text-ink-600 transition-colors hover:bg-ink-900/5 hover:text-ink-900"
                >
                  {link.label}
                </a>
              </li>
            ))}
          </ul>
        </nav>
        <div className="flex items-center gap-1.5">
          {signedIn ? (
            <Button asChild size="sm" variant="ink" className="hidden sm:inline-flex">
              <Link href="/app">
                Open your account <ArrowRight aria-hidden />
              </Link>
            </Button>
          ) : (
            <>
              <Button asChild size="sm" variant="ghost" className="hidden sm:inline-flex">
                <Link href="/login">Sign in</Link>
              </Button>
              <Button asChild size="sm" className="hidden sm:inline-flex">
                <Link href="/register">Create account</Link>
              </Button>
            </>
          )}
          <Dialog.Root open={open} onOpenChange={setOpen}>
            <Dialog.Trigger asChild>
              <Button variant="ghost" size="icon" className="lg:hidden" aria-label="Open menu">
                <Menu aria-hidden />
              </Button>
            </Dialog.Trigger>
            <Dialog.Portal>
              <Dialog.Overlay className="fixed inset-0 z-50 bg-ink-950/40 backdrop-blur-sm data-[state=open]:animate-fade" />
              <Dialog.Content className="fixed inset-x-3 top-3 z-50 rounded-feature bg-paper p-5 shadow-pop data-[state=open]:animate-rise">
                <div className="flex items-center justify-between">
                  <Logo />
                  <Dialog.Close asChild>
                    <Button variant="ghost" size="icon" aria-label="Close menu">
                      <X aria-hidden />
                    </Button>
                  </Dialog.Close>
                </div>
                <Dialog.Title className="sr-only">Menu</Dialog.Title>
                <Dialog.Description className="sr-only">Site navigation</Dialog.Description>
                <nav aria-label="Mobile" className="mt-6">
                  <ul className="flex flex-col">
                    {links.map((link, index) => (
                      <li key={link.href} className="animate-rise" style={{ animationDelay: `${60 + index * 40}ms` }}>
                        <a
                          href={link.href}
                          onClick={() => setOpen(false)}
                          className="flex items-center justify-between rounded-control px-3 py-3.5 font-display text-xl font-medium text-ink-900 transition-colors hover:bg-paper-2"
                        >
                          {link.label}
                          <ArrowRight aria-hidden className="size-4 text-ink-400" />
                        </a>
                      </li>
                    ))}
                  </ul>
                </nav>
                <div className="mt-6 grid gap-2">
                  {signedIn ? (
                    <Button asChild size="lg" variant="ink">
                      <Link href="/app">Open your account</Link>
                    </Button>
                  ) : (
                    <>
                      <Button asChild size="lg">
                        <Link href="/register">Create account</Link>
                      </Button>
                      <Button asChild size="lg" variant="quiet">
                        <Link href="/login">Sign in</Link>
                      </Button>
                    </>
                  )}
                </div>
              </Dialog.Content>
            </Dialog.Portal>
          </Dialog.Root>
        </div>
      </div>
    </header>
  );
}
