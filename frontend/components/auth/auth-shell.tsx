import { Fingerprint, KeyRound, MonitorSmartphone } from "lucide-react";
import Link from "next/link";
import { Logo, LogoMark } from "@/components/brand/logo";

const points = [
  { icon: Fingerprint, text: "Your email is verified before you can sign in." },
  { icon: KeyRound, text: "Passwords are hashed with Argon2id and never stored in plain text." },
  { icon: MonitorSmartphone, text: "See and sign out every session from your account." },
];

/** Shared frame for sign-in, sign-up, verification and recovery. */
export function AuthShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-dvh lg:grid lg:grid-cols-[minmax(0,0.92fr)_minmax(0,1.08fr)]">
      <aside className="relative isolate hidden overflow-hidden bg-ink-950 text-paper lg:flex lg:flex-col lg:justify-between lg:p-12 xl:p-16">
        <div aria-hidden className="absolute -left-24 -top-24 -z-10 size-[28rem] animate-breathe rounded-full bg-brand-600/40 blur-[110px]" />
        <div aria-hidden className="absolute -bottom-40 right-0 -z-10 size-[26rem] rounded-full bg-brand-800/40 blur-[110px]" />
        <Link href="/" className="inline-flex w-fit items-center gap-2.5 rounded-lg text-white">
          <LogoMark />
          <span className="font-display text-[1.15rem] font-semibold tracking-[-0.03em]">DEYPAY</span>
        </Link>
        <div className="max-w-md">
          <p className="font-display text-[2.6rem] font-semibold leading-[1.06] text-white xl:text-5xl">
            Digital dollars, with every step verified.
          </p>
          <ul className="mt-10 grid gap-5">
            {points.map(({ icon: Icon, text }) => (
              <li key={text} className="flex items-start gap-3.5 text-ink-300">
                <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-full bg-white/[0.07] text-brand-300">
                  <Icon aria-hidden className="size-[1.05rem]" />
                </span>
                <span className="pt-1.5 leading-relaxed">{text}</span>
              </li>
            ))}
          </ul>
        </div>
        <p className="text-sm text-ink-400">DEYPAY is a product of Kenule Africa.</p>
      </aside>
      <div className="flex min-h-dvh flex-col px-5 pb-10 pt-5 sm:px-8">
        <header className="flex items-center justify-between lg:justify-end">
          <Logo className="lg:hidden" />
          <Link href="/" className="rounded-md text-sm text-ink-500 transition-colors hover:text-ink-900">
            Back to home
          </Link>
        </header>
        <main id="main" className="mx-auto flex w-full max-w-form flex-1 flex-col justify-center py-10 sm:py-14">
          {children}
        </main>
      </div>
    </div>
  );
}

export function AuthHeading({ title, description }: { title: string; description?: React.ReactNode }) {
  return (
    <div className="animate-rise">
      <h1 className="font-display text-[2rem] font-semibold leading-tight text-ink-950 sm:text-4xl">{title}</h1>
      {description && <p className="mt-3 text-[1.0625rem] leading-relaxed text-ink-500">{description}</p>}
    </div>
  );
}
