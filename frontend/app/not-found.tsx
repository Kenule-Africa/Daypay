import { ArrowLeft } from "lucide-react";
import Link from "next/link";
import { Logo } from "@/components/brand/logo";
import { Button } from "@/components/ui/button";

export default function NotFound() {
  return (
    <main className="mx-auto flex min-h-dvh max-w-form flex-col px-5 py-6">
      <Logo />
      <div className="my-auto animate-rise py-16">
        <p className="font-display text-7xl font-semibold tracking-tight text-brand-500">404</p>
        <h1 className="mt-4 font-display text-3xl font-semibold text-ink-950">This page doesn&apos;t exist</h1>
        <p className="mt-3 text-[1.0625rem] leading-relaxed text-ink-600">
          The link may be mistyped, or the page may have moved.
        </p>
        <Button asChild size="lg" className="mt-8">
          <Link href="/">
            <ArrowLeft aria-hidden /> Back to home
          </Link>
        </Button>
      </div>
    </main>
  );
}
