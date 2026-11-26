"use client";

import {
  ArrowLeftRight,
  ArrowRight,
  Banknote,
  CircleCheck,
  CircleDashed,
  Clock,
  ImageIcon,
  Landmark,
  MailCheck,
  ScanFace,
  Send,
  Smartphone,
  Wallet,
  Zap,
} from "lucide-react";
import Link from "next/link";
import { useAccount } from "@/components/account/account-context";
import { Avatar } from "@/components/account/avatar";
import { Badge } from "@/components/ui/badge";
import { accountStatus, formatDay } from "@/lib/account";
import { cn } from "@/lib/utils";

const statusToneClass = {
  positive: "bg-positive-soft text-positive",
  caution: "bg-caution-soft text-caution",
  danger: "bg-danger-soft text-danger",
  neutral: "bg-paper-2 text-ink-600",
};

const upcoming = [
  { icon: Wallet, title: "USDC and USDT wallets", text: "Hold digital dollars with clear available and reserved amounts." },
  { icon: Send, title: "Send and receive", text: "Transfer to other DEYPAY customers by username." },
  { icon: ArrowLeftRight, title: "Convert", text: "Move between stablecoins and naira with a quote first." },
  { icon: Landmark, title: "Fund and withdraw in naira", text: "Use supported Nigerian payment methods and banks." },
  { icon: Banknote, title: "Pay Anyone", text: "Pay any Nigerian bank account from your stablecoins." },
  { icon: Zap, title: "Auto-Settlement", text: "Settle confirmed USDC deposits to your bank automatically." },
];

export function Overview() {
  const { profile } = useAccount();
  const status = accountStatus[profile.account_status];
  const steps = [
    {
      icon: MailCheck,
      title: "Confirm your email",
      done: profile.email_verified,
      detail: profile.email_verified ? profile.email : "Check your inbox for the verification link.",
    },
    {
      icon: ImageIcon,
      title: "Add a profile photo",
      done: profile.profile_image !== null,
      detail: profile.profile_image ? "Your photo is set." : "Optional. Helps people recognise you.",
      href: "/app/profile",
    },
    {
      icon: Smartphone,
      title: "Verify your phone",
      done: profile.phone_verified,
      detail: profile.phone_verified ? profile.phone : "Phone verification isn't available yet.",
      unavailable: !profile.phone_verified,
    },
    {
      icon: ScanFace,
      title: "Verify your identity",
      done: profile.account_status === "kyc_approved",
      detail:
        profile.account_status === "kyc_approved"
          ? "Your identity is verified."
          : "Needed for financial features. Not available in the app yet.",
      unavailable: profile.account_status !== "kyc_approved",
    },
  ];
  const completed = steps.filter(step => step.done).length;

  return (
    <div className="grid gap-5 lg:gap-6">
      {/* Welcome */}
      <section className="relative isolate overflow-hidden rounded-hero bg-surface p-6 shadow-soft sm:p-9">
        <div aria-hidden className="absolute -right-24 -top-24 -z-10 size-72 rounded-full bg-brand-100 blur-3xl" />
        <div className="flex flex-col gap-6 sm:flex-row sm:items-center">
          <Avatar profile={profile} className="size-16 text-xl sm:size-20 sm:text-2xl" />
          <div className="min-w-0 flex-1">
            <p className="text-[0.9375rem] text-ink-500">Welcome back,</p>
            <h1 className="truncate font-display text-3xl font-semibold leading-tight text-ink-950 sm:text-[2.6rem]">
              {profile.first_name}
            </h1>
            <p className="mt-1 text-[0.9375rem] text-ink-500">
              {`@${profile.username} · Member since ${formatDay(profile.created_at)}`}
            </p>
          </div>
        </div>
        <div className="mt-7 flex flex-col gap-3 rounded-panel bg-paper p-4 sm:flex-row sm:items-center sm:p-5">
          <span className={cn("inline-flex w-fit items-center gap-2 rounded-full px-3 py-1 text-sm font-semibold", statusToneClass[status.tone])}>
            <CircleCheck aria-hidden className="size-4" />
            {status.label}
          </span>
          <p className="text-[0.9375rem] leading-relaxed text-ink-600">{status.detail}</p>
        </div>
      </section>

      <div className="grid gap-5 lg:grid-cols-[1.05fr_0.95fr] lg:gap-6">
        {/* Setup progress */}
        <section className="rounded-feature bg-surface p-5 shadow-soft sm:p-7">
          <div className="flex items-center justify-between gap-3">
            <h2 className="font-display text-lg font-semibold text-ink-900">Account setup</h2>
            <span className="text-sm tabular-nums text-ink-500">
              {completed} of {steps.length} complete
            </span>
          </div>
          <div
            className="mt-4 h-1.5 overflow-hidden rounded-full bg-paper-2"
            role="progressbar"
            aria-label="Account setup progress"
            aria-valuemin={0}
            aria-valuemax={steps.length}
            aria-valuenow={completed}
          >
            <div
              className="h-full rounded-full bg-brand-500 transition-[width] duration-700 ease-out-expo"
              style={{ width: `${(completed / steps.length) * 100}%` }}
            />
          </div>
          <ul className="mt-5 grid gap-1">
            {steps.map(({ icon: Icon, title, detail, done, href, unavailable }) => {
              const content = (
                <>
                  <span
                    className={cn(
                      "inline-flex size-10 shrink-0 items-center justify-center rounded-full",
                      done ? "bg-positive-soft text-positive" : "bg-paper-2 text-ink-500",
                    )}
                  >
                    {done ? <CircleCheck aria-hidden className="size-5" /> : <Icon aria-hidden className="size-5" />}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className={cn("block font-medium", done ? "text-ink-500" : "text-ink-900")}>{title}</span>
                    <span className="block truncate text-sm text-ink-500">{detail}</span>
                  </span>
                  {done ? (
                    <span className="sr-only">Complete</span>
                  ) : unavailable ? (
                    <Badge tone="soon">
                      <Clock aria-hidden /> Later
                    </Badge>
                  ) : href ? (
                    <ArrowRight aria-hidden className="size-4 text-ink-400 transition-transform group-hover:translate-x-0.5" />
                  ) : (
                    <CircleDashed aria-hidden className="size-4 text-ink-300" />
                  )}
                </>
              );
              return (
                <li key={title}>
                  {href && !done ? (
                    <Link href={href} className="group flex items-center gap-4 rounded-panel p-2.5 transition-colors hover:bg-paper">
                      {content}
                    </Link>
                  ) : (
                    <div className="flex items-center gap-4 p-2.5">{content}</div>
                  )}
                </li>
              );
            })}
          </ul>
        </section>

        {/* Financial features: honest unavailable state */}
        <section aria-labelledby="money-title" className="relative isolate overflow-hidden rounded-feature bg-ink-950 p-5 text-paper shadow-pop sm:p-7">
          <div aria-hidden className="absolute -bottom-24 -right-20 -z-10 size-64 rounded-full bg-brand-600/30 blur-[80px]" />
          <div className="flex items-center justify-between gap-3">
            <h2 id="money-title" className="font-display text-lg font-semibold text-white">
              Money features
            </h2>
            <Badge tone="dark">Not yet available</Badge>
          </div>
          <p className="mt-2 text-[0.9375rem] leading-relaxed text-ink-300">
            Wallets and payments aren&apos;t open yet. You won&apos;t see balances or activity here until they are real.
          </p>
          <ul className="mt-5 grid gap-1">
            {upcoming.map(({ icon: Icon, title, text }) => (
              <li key={title} className="flex items-start gap-3.5 rounded-panel p-2.5">
                <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-full bg-white/[0.07] text-brand-300">
                  <Icon aria-hidden className="size-[1.05rem]" />
                </span>
                <span>
                  <span className="block text-[0.9375rem] font-medium text-white">{title}</span>
                  <span className="block text-sm leading-snug text-ink-400">{text}</span>
                </span>
              </li>
            ))}
          </ul>
        </section>
      </div>

      {/* Shortcuts */}
      <nav aria-label="Account shortcuts" className="grid gap-3 sm:grid-cols-3">
        {[
          { href: "/app/profile", title: "Profile", text: "Name, photo and contact details" },
          { href: "/app/preferences", title: "Preferences", text: "Notifications and discovery" },
          { href: "/app/security", title: "Security", text: "Sessions and password" },
        ].map(link => (
          <Link
            key={link.href}
            href={link.href}
            className="group flex items-center justify-between gap-3 rounded-feature bg-surface p-5 shadow-soft transition-[box-shadow,transform] duration-300 ease-out-expo hover:-translate-y-0.5 hover:shadow-lift"
          >
            <span>
              <span className="block font-display font-semibold text-ink-900">{link.title}</span>
              <span className="block text-sm text-ink-500">{link.text}</span>
            </span>
            <ArrowRight aria-hidden className="size-4 text-ink-400 transition-transform duration-300 group-hover:translate-x-0.5 group-hover:text-brand-500" />
          </Link>
        ))}
      </nav>
    </div>
  );
}
