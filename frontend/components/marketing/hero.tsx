import { ArrowDown, ArrowRight, Fingerprint, KeyRound, MonitorSmartphone } from "lucide-react";
import Link from "next/link";
import { LogoMark } from "@/components/brand/logo";
import { Badge, LiveDot } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

const assurances = [
  { icon: Fingerprint, label: "Email-verified sign-in" },
  { icon: KeyRound, label: "Argon2id password protection" },
  { icon: MonitorSmartphone, label: "Control every session" },
];

export function Hero({ signedIn }: { signedIn: boolean }) {
  return (
    <section className="relative px-4 pb-20 pt-10 sm:px-6 sm:pt-16 lg:pb-28 lg:pt-20">
      <div className="mx-auto grid max-w-site items-center gap-14 lg:grid-cols-[1.08fr_0.92fr] lg:gap-10">
        <div className="max-w-2xl">
          <p className="inline-flex animate-rise items-center gap-2.5 rounded-full bg-surface py-1.5 pl-3 pr-4 text-[0.8125rem] text-ink-600 shadow-soft">
            <span className="text-brand-500">
              <LiveDot />
            </span>
            <span>
              <span className="font-medium text-ink-900">Accounts come first.</span> Wallets and payments are in development.
            </span>
          </p>
          <h1
            className="mt-7 animate-rise font-display text-[2.75rem] font-semibold leading-[1.02] text-ink-950 [animation-delay:80ms] sm:text-6xl lg:text-[4.6rem]"
          >
            Digital dollars that work like{" "}
            <span className="relative whitespace-nowrap text-brand-500">
              everyday money
              <svg aria-hidden viewBox="0 0 300 12" preserveAspectRatio="none" className="absolute -bottom-1.5 left-0 h-2.5 w-full text-brand-300">
                <path d="M2 9C60 3 140 2 298 6" fill="none" stroke="currentColor" strokeWidth="4" strokeLinecap="round" />
              </svg>
            </span>
            .
          </h1>
          <p className="mt-7 max-w-xl animate-rise text-lg leading-relaxed text-ink-600 [animation-delay:160ms] sm:text-xl">
            DEYPAY is a stablecoin-first account built for Nigeria. Hold USDC and USDT, send to other DEYPAY
            customers, and pay any Nigerian bank account in naira — without needing to understand blockchains.
          </p>
          <div className="mt-9 flex animate-rise flex-col gap-3 [animation-delay:240ms] sm:flex-row">
            {signedIn ? (
              <Button asChild size="lg">
                <Link href="/app">
                  Go to your account <ArrowRight aria-hidden />
                </Link>
              </Button>
            ) : (
              <Button asChild size="lg">
                <Link href="/register">
                  Create your account <ArrowRight aria-hidden />
                </Link>
              </Button>
            )}
            <Button asChild size="lg" variant="quiet">
              <a href="#product">
                See how it works <ArrowDown aria-hidden />
              </a>
            </Button>
          </div>
          <ul className="mt-10 flex animate-rise flex-wrap gap-x-6 gap-y-3 [animation-delay:320ms]">
            {assurances.map(({ icon: Icon, label }) => (
              <li key={label} className="flex items-center gap-2 text-sm text-ink-600">
                <Icon aria-hidden className="size-4 text-brand-500" />
                {label}
              </li>
            ))}
          </ul>
        </div>
        <MoneyFlow />
      </div>
    </section>
  );
}

/** Explains the intended product flow. It shows no amounts, balances or activity. */
function MoneyFlow() {
  return (
    <figure
      aria-labelledby="flow-caption"
      className="relative isolate mx-auto w-full max-w-[34rem] animate-rise overflow-hidden rounded-hero bg-ink-900 p-6 text-paper shadow-pop [animation-delay:200ms] sm:p-8"
    >
      <div aria-hidden className="pointer-events-none absolute -right-24 -top-28 -z-10 size-80 animate-breathe rounded-full bg-brand-500/45 blur-[90px]" />
      <div aria-hidden className="pointer-events-none absolute -bottom-32 -left-20 -z-10 size-72 rounded-full bg-brand-700/30 blur-[90px]" />
      <svg aria-hidden className="pointer-events-none absolute inset-0 -z-10 size-full opacity-[0.07]">
        <defs>
          <pattern id="flow-grid" width="28" height="28" patternUnits="userSpaceOnUse">
            <path d="M28 0H0V28" fill="none" stroke="white" strokeWidth="1" />
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill="url(#flow-grid)" />
      </svg>

      <div className="flex items-center justify-between gap-3">
        <p className="text-xs font-medium uppercase tracking-[0.16em] text-ink-300">How DEYPAY moves money</p>
        <Badge tone="dark">In development</Badge>
      </div>

      <ol className="mt-7 flex flex-col">
        <FlowStep
          index="01"
          title="Your digital dollars"
          detail="USDC and USDT wallet profiles, created after verification."
          aside={
            <span className="flex gap-1.5">
              <Token>USDC</Token>
              <Token>USDT</Token>
            </span>
          }
        />
        <Connector />
        <FlowStep
          index="02"
          title="DEYPAY converts"
          detail="A quote to review before you confirm."
          aside={<LogoMark className="size-9" />}
          highlight
        />
        <Connector />
        <FlowStep
          index="03"
          title="Any Nigerian bank"
          detail="Your recipient receives naira by bank transfer."
          aside={<Token>NGN</Token>}
        />
      </ol>
      <figcaption id="flow-caption" className="mt-7 text-[0.8125rem] leading-relaxed text-ink-300">
        The planned Pay Anyone flow. Money features open after identity verification and the required approvals.
      </figcaption>
    </figure>
  );
}

function FlowStep({
  index,
  title,
  detail,
  aside,
  highlight = false,
}: {
  index: string;
  title: string;
  detail: string;
  aside: React.ReactNode;
  highlight?: boolean;
}) {
  return (
    <li
      className={
        highlight
          ? "flex items-center gap-4 rounded-panel bg-brand-500/20 p-4 sm:p-5"
          : "flex items-center gap-4 rounded-panel bg-white/[0.045] p-4 sm:p-5"
      }
    >
      <span className="font-display text-sm tabular-nums text-ink-400">{index}</span>
      <div className="min-w-0 flex-1">
        <p className="font-display text-[1.05rem] font-semibold text-white">{title}</p>
        <p className="mt-0.5 text-sm leading-snug text-ink-300">{detail}</p>
      </div>
      {aside}
    </li>
  );
}

function Token({ children }: { children: React.ReactNode }) {
  return (
    <span className="rounded-full bg-white/10 px-2.5 py-1 font-display text-xs font-semibold tracking-wide text-white">
      {children}
    </span>
  );
}

function Connector() {
  return (
    <li aria-hidden className="flex h-9 pl-[1.65rem] sm:pl-[1.9rem]">
      <svg viewBox="0 0 2 36" preserveAspectRatio="none" className="h-full w-0.5 overflow-visible">
        <line x1="1" y1="0" x2="1" y2="36" stroke="rgb(255 255 255 / 0.12)" strokeWidth="2" />
        <line
          x1="1"
          y1="0"
          x2="1"
          y2="36"
          stroke="var(--color-brand-400)"
          strokeWidth="2"
          strokeDasharray="6 14"
          strokeLinecap="round"
          className="animate-flow"
        />
      </svg>
    </li>
  );
}
