"use client";

import { RotateCw, ServerCrash } from "lucide-react";
import { Button } from "@/components/ui/button";

export default function AccountError({ reset }: { error: Error; reset: () => void }) {
  return (
    <div role="alert" className="mx-auto max-w-form animate-rise py-16 text-center">
      <span className="inline-flex size-14 items-center justify-center rounded-feature bg-caution-soft text-caution">
        <ServerCrash aria-hidden className="size-7" />
      </span>
      <h1 className="mt-6 font-display text-2xl font-semibold text-ink-950">This page couldn&apos;t load</h1>
      <p className="mt-2 text-ink-600">DEYPAY&apos;s account service didn&apos;t respond. Please try again.</p>
      <Button className="mt-7" onClick={reset}>
        <RotateCw aria-hidden /> Try again
      </Button>
    </div>
  );
}
