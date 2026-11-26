import { Activity } from "lucide-react";
import type { Metadata } from "next";
import { Logo } from "@/components/brand/logo";
import { apiBase } from "@/lib/server/session";
import { cn } from "@/lib/utils";

export const metadata: Metadata = { title: "Service status" };

async function apiAvailable(): Promise<boolean> {
  try {
    const response = await fetch(new URL("/health/ready", apiBase()), {
      cache: "no-store",
      redirect: "error",
      signal: AbortSignal.timeout(2500),
    });
    return response.ok && (await response.json()).status === "ready";
  } catch {
    return false;
  }
}

export default async function StatusPage() {
  const ready = await apiAvailable();
  return (
    <main className="mx-auto flex min-h-dvh max-w-reading flex-col px-5 py-8">
      <header>
        <Logo />
      </header>
      <section aria-labelledby="status-title" className="my-auto py-16">
        <Activity aria-hidden className="size-7 text-brand-500" />
        <h1 id="status-title" className="mt-5 font-display text-4xl font-semibold text-ink-950">
          Service status
        </h1>
        <dl className="mt-8 flex items-center justify-between rounded-feature bg-surface p-6 shadow-soft">
          <dt className="font-medium text-ink-800">Payments API</dt>
          <dd
            data-ready={ready}
            className={cn(
              "rounded-full px-3 py-1 text-sm font-medium",
              ready ? "bg-positive-soft text-positive" : "bg-danger-soft text-danger",
            )}
          >
            {ready ? "Available" : "Unavailable"}
          </dd>
        </dl>
      </section>
    </main>
  );
}
