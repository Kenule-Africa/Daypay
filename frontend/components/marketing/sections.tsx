import {
  ArrowLeftRight,
  ArrowRight,
  BadgeCheck,
  Banknote,
  Coins,
  Fingerprint,
  KeyRound,
  Landmark,
  Laptop,
  MonitorSmartphone,
  RefreshCcw,
  ScanFace,
  Send,
  ServerCog,
  ShieldCheck,
  Store,
  Users,
  Wallet,
  Zap,
} from "lucide-react";
import Link from "next/link";
import { Reveal } from "@/components/motion/reveal";
import { Badge, LiveDot } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

function Eyebrow({ children, dark = false }: { children: React.ReactNode; dark?: boolean }) {
  return (
    <p className={`text-xs font-semibold uppercase tracking-[0.18em] ${dark ? "text-brand-300" : "text-brand-600"}`}>
      {children}
    </p>
  );
}

const audiences = [
  { icon: Laptop, title: "Freelancers and remote workers", text: "Receive digital dollars from clients and spend them at home." },
  { icon: Store, title: "Entrepreneurs and online businesses", text: "Pay suppliers and staff in naira from stablecoin funds." },
  { icon: Coins, title: "Digital asset users", text: "Bring USDC and USDT into an account that connects to Nigerian banks." },
  { icon: Users, title: "Everyday payments", text: "Send to friends and family on DEYPAY, or to any Nigerian bank account." },
];

export function Purpose() {
  return (
    <section id="product" className="scroll-mt-24 px-4 py-20 sm:px-6 lg:py-28">
      <div className="mx-auto max-w-site">
        <Reveal className="grid gap-8 lg:grid-cols-[0.9fr_1.1fr] lg:gap-20">
          <div>
            <Eyebrow>What DEYPAY is</Eyebrow>
            <h2 className="mt-4 font-display text-4xl font-semibold leading-[1.05] text-ink-950 sm:text-5xl">
              One account.
              <br />
              <span className="text-ink-400">Many ways to move money.</span>
            </h2>
          </div>
          <p className="self-end text-lg leading-relaxed text-ink-600 sm:text-xl">
            DEYPAY takes the complexity out of digital dollars. You hold, receive, send and spend USDC and USDT as
            simply as you would use a banking app, while naira handles funding, conversion and payments to Nigerian
            banks.
          </p>
        </Reveal>
        <ul className="mt-16 grid gap-x-10 gap-y-10 sm:grid-cols-2 lg:mt-20 lg:grid-cols-4">
          {audiences.map(({ icon: Icon, title, text }, index) => (
            <Reveal as="li" key={title} delay={index * 70}>
              <span className="inline-flex size-11 items-center justify-center rounded-[0.9rem] bg-brand-50 text-brand-600">
                <Icon aria-hidden className="size-5" />
              </span>
              <h3 className="mt-5 font-display text-lg font-semibold text-ink-900">{title}</h3>
              <p className="mt-2 text-[0.9375rem] leading-relaxed text-ink-500">{text}</p>
            </Reveal>
          ))}
        </ul>
      </div>
    </section>
  );
}

export function Wallets() {
  return (
    <section className="px-4 pb-20 sm:px-6 lg:pb-28">
      <div className="mx-auto grid max-w-site gap-6 lg:grid-cols-[1fr_1fr_0.95fr]">
        <Reveal className="flex flex-col justify-between rounded-hero bg-surface p-7 shadow-soft sm:p-9 lg:col-span-2 lg:flex-row lg:gap-10">
          <div className="max-w-md">
            <div className="flex items-center gap-3">
              <Wallet aria-hidden className="size-5 text-brand-500" />
              <Badge tone="soon">In development</Badge>
            </div>
            <h2 className="mt-6 font-display text-3xl font-semibold leading-tight text-ink-950 sm:text-4xl">
              USDC and USDT, side by side.
            </h2>
            <p className="mt-4 text-[1.0625rem] leading-relaxed text-ink-600">
              After identity verification, every eligible customer receives USDC and USDT wallet profiles. Deposit
              from supported networks, withdraw to external wallets, and always see what is available and what is
              reserved.
            </p>
          </div>
          <div className="mt-10 grid grid-cols-2 gap-3 self-end lg:mt-0 lg:w-[19rem] lg:grid-cols-1">
            <AssetTile code="USDC" name="USD Coin" />
            <AssetTile code="USDT" name="Tether USD" />
          </div>
        </Reveal>
        <Reveal delay={90} className="rounded-hero bg-brand-500 p-7 text-white shadow-glow sm:p-9">
          <ShieldCheck aria-hidden className="size-6 text-brand-100" />
          <p className="mt-6 font-display text-2xl font-semibold leading-snug">
            Naira moves money. It isn&apos;t held as a wallet balance.
          </p>
          <p className="mt-4 leading-relaxed text-brand-100">
            DEYPAY uses naira for funding, conversion, bank payments and withdrawals. Your stored value stays in
            digital dollars.
          </p>
        </Reveal>
      </div>
    </section>
  );
}

function AssetTile({ code, name }: { code: string; name: string }) {
  return (
    <div className="rounded-panel bg-paper p-5">
      <p className="font-display text-3xl font-semibold tracking-tight text-ink-950 sm:text-4xl">{code}</p>
      <p className="mt-1 text-sm text-ink-500">{name} · stablecoin</p>
    </div>
  );
}

const movements = [
  {
    icon: Send,
    title: "Send to DEYPAY customers",
    text: "Transfer stablecoins to another customer by username, recorded in DEYPAY's own ledger.",
  },
  {
    icon: ArrowLeftRight,
    title: "Convert with a clear quote",
    text: "Move between USDC, USDT and naira. Review the quote before you confirm.",
  },
  {
    icon: Landmark,
    title: "Fund and withdraw in naira",
    text: "Add money through supported Nigerian payment methods and withdraw to your bank.",
  },
];

export function MoveMoney() {
  return (
    <section className="px-4 pb-20 sm:px-6 lg:pb-28">
      <div className="mx-auto max-w-site">
        <Reveal className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <Eyebrow>Transfers and conversion</Eyebrow>
            <h2 className="mt-4 max-w-xl font-display text-4xl font-semibold leading-[1.08] text-ink-950 sm:text-5xl">
              Move value without the jargon.
            </h2>
          </div>
          <Badge tone="soon" className="self-start sm:self-auto">
            In development
          </Badge>
        </Reveal>
        <ol className="mt-14 grid gap-10 lg:grid-cols-3 lg:gap-0">
          {movements.map(({ icon: Icon, title, text }, index) => (
            <Reveal as="li" key={title} delay={index * 90} className="relative lg:pr-12">
              <div className="flex items-center gap-4">
                <span className="inline-flex size-12 items-center justify-center rounded-full bg-ink-900 text-white">
                  <Icon aria-hidden className="size-5" />
                </span>
                {index < movements.length - 1 && (
                  <span aria-hidden className="hidden h-px flex-1 bg-linear-to-r from-ink-300 to-transparent lg:block" />
                )}
              </div>
              <p className="mt-6 font-display text-sm tabular-nums text-ink-400">0{index + 1}</p>
              <h3 className="mt-1 font-display text-xl font-semibold text-ink-900">{title}</h3>
              <p className="mt-2 max-w-sm leading-relaxed text-ink-500">{text}</p>
            </Reveal>
          ))}
        </ol>
      </div>
    </section>
  );
}

export function PayAnyone() {
  return (
    <section id="pay-anyone" className="scroll-mt-24 px-3 sm:px-5">
      <div className="relative isolate mx-auto max-w-[84rem] overflow-hidden rounded-hero bg-ink-950 px-5 py-20 text-paper sm:px-10 lg:px-16 lg:py-28">
        <div aria-hidden className="absolute -left-40 top-1/3 -z-10 size-[30rem] rounded-full bg-brand-600/25 blur-[120px]" />
        <div className="mx-auto grid max-w-site items-center gap-14 lg:grid-cols-2 lg:gap-20">
          <Reveal>
            <div className="flex items-center gap-3">
              <Eyebrow dark>Pay Anyone</Eyebrow>
              <Badge tone="dark">In development</Badge>
            </div>
            <h2 className="mt-5 font-display text-4xl font-semibold leading-[1.06] text-white sm:text-5xl">
              Pay any Nigerian bank account with digital dollars.
            </h2>
            <p className="mt-6 max-w-lg text-lg leading-relaxed text-ink-300">
              Fund a payment with USDC or USDT. DEYPAY converts what&apos;s needed and sends naira by bank transfer.
              Your recipient doesn&apos;t need a DEYPAY account, a crypto wallet or any knowledge of blockchains.
            </p>
          </Reveal>
          <Reveal delay={120}>
            <dl className="rounded-feature bg-white/[0.05] p-2 backdrop-blur">
              <SlipRow icon={Coins} term="You pay with" detail="USDC or USDT from your DEYPAY wallet" />
              <SlipRow icon={BadgeCheck} term="Before you pay" detail="The recipient's bank account is verified" />
              <SlipRow icon={RefreshCcw} term="DEYPAY handles" detail="Conversion to naira through approved partners" />
              <SlipRow icon={Banknote} term="They receive" detail="Naira in their Nigerian bank account" accent />
            </dl>
          </Reveal>
        </div>
      </div>
    </section>
  );
}

function SlipRow({
  icon: Icon,
  term,
  detail,
  accent = false,
}: {
  icon: typeof Coins;
  term: string;
  detail: string;
  accent?: boolean;
}) {
  return (
    <div className={`flex items-center gap-4 rounded-panel p-4 sm:p-5 ${accent ? "bg-brand-500 text-white" : ""}`}>
      <span
        className={`inline-flex size-10 shrink-0 items-center justify-center rounded-full ${accent ? "bg-white/15" : "bg-white/[0.07] text-brand-300"}`}
      >
        <Icon aria-hidden className="size-[1.1rem]" />
      </span>
      <div>
        <dt className={`text-xs font-medium uppercase tracking-[0.14em] ${accent ? "text-brand-100" : "text-ink-400"}`}>
          {term}
        </dt>
        <dd className="mt-0.5 font-display text-[1.05rem] font-medium text-white">{detail}</dd>
      </div>
    </div>
  );
}

const settlementSteps = [
  { title: "Link your bank", text: "A dedicated deposit address is mapped to your verified Nigerian bank account." },
  { title: "Deposit is confirmed", text: "DEYPAY waits for the required network confirmations before acting." },
  { title: "One conversion, one payout", text: "The deposit is converted once and naira is paid to your linked account." },
];

export function AutoSettlement() {
  return (
    <section id="auto-settlement" className="scroll-mt-24 px-4 py-20 sm:px-6 lg:py-28">
      <div className="mx-auto grid max-w-site gap-12 lg:grid-cols-[0.85fr_1.15fr] lg:gap-20">
        <Reveal>
          <div className="flex items-center gap-3">
            <Eyebrow>Auto-Settlement</Eyebrow>
            <Badge tone="soon">Coming later</Badge>
          </div>
          <h2 className="mt-5 font-display text-4xl font-semibold leading-[1.06] text-ink-950 sm:text-5xl">
            Receive USDC. Settle to your bank automatically.
          </h2>
          <p className="mt-6 text-lg leading-relaxed text-ink-600">
            For people who are paid in digital dollars but spend in naira. Once activated, confirmed USDC deposits
            settle to your bank without you converting each one by hand.
          </p>
        </Reveal>
        <ol className="relative grid gap-3">
          {settlementSteps.map((step, index) => (
            <Reveal
              as="li"
              key={step.title}
              delay={index * 90}
              className="group flex gap-5 rounded-feature bg-surface p-6 shadow-soft transition-[transform,box-shadow] duration-500 ease-out-expo hover:-translate-y-0.5 hover:shadow-lift sm:p-7"
            >
              <span className="inline-flex size-10 shrink-0 items-center justify-center rounded-full bg-brand-50 font-display text-sm font-semibold text-brand-700 transition-colors duration-300 group-hover:bg-brand-500 group-hover:text-white">
                {index + 1}
              </span>
              <div>
                <h3 className="font-display text-lg font-semibold text-ink-900">{step.title}</h3>
                <p className="mt-1.5 leading-relaxed text-ink-500">{step.text}</p>
              </div>
            </Reveal>
          ))}
        </ol>
      </div>
    </section>
  );
}

const safeguards = [
  { icon: Fingerprint, title: "Verified email before sign-in", text: "New accounts confirm their email address before they can sign in." },
  { icon: KeyRound, title: "Strong password protection", text: "Passwords are hashed with Argon2id and checked against known-compromised passwords." },
  { icon: ServerCog, title: "An isolated sign-in service", text: "Credentials are checked by a dedicated authentication service, separate from the rest of DEYPAY." },
  { icon: MonitorSmartphone, title: "Your sessions, your control", text: "See every signed-in session and sign any of them out at once." },
  { icon: Zap, title: "Limits on repeated attempts", text: "Sign-in, recovery and sign-up are rate limited to slow down automated abuse." },
  { icon: ScanFace, title: "Identity checks before money moves", text: "Financial features will require identity verification and eligibility checks." },
];

export function Security() {
  return (
    <section id="security" className="scroll-mt-24 bg-paper-2 px-4 py-20 sm:px-6 lg:py-28">
      <div className="mx-auto grid max-w-site gap-14 lg:grid-cols-[0.8fr_1.2fr] lg:gap-20">
        <Reveal className="lg:sticky lg:top-28 lg:self-start">
          <Eyebrow>Security and trust</Eyebrow>
          <h2 className="mt-4 font-display text-4xl font-semibold leading-[1.06] text-ink-950 sm:text-5xl">
            Security you can see from day one.
          </h2>
          <p className="mt-6 text-lg leading-relaxed text-ink-600">
            These protections are built into every DEYPAY account. We describe only what exists, and we say plainly
            when something is still on the way.
          </p>
        </Reveal>
        <ul className="grid gap-x-10 gap-y-11 sm:grid-cols-2">
          {safeguards.map(({ icon: Icon, title, text }, index) => (
            <Reveal as="li" key={title} delay={(index % 2) * 80}>
              <Icon aria-hidden className="size-6 text-brand-500" />
              <h3 className="mt-4 font-display text-lg font-semibold text-ink-900">{title}</h3>
              <p className="mt-2 leading-relaxed text-ink-500">{text}</p>
            </Reveal>
          ))}
        </ul>
      </div>
    </section>
  );
}

const available = [
  "Create a DEYPAY account",
  "Email verification",
  "Profile, photo and preferences",
  "Session and sign-in security",
  "Password recovery by email",
];

const upcoming = [
  "USDC and USDT wallets",
  "Transfers between DEYPAY customers",
  "Stablecoin and naira conversion",
  "Naira funding and withdrawals",
  "Pay Anyone bank payments",
  "Auto-Settlement",
];

export function Availability() {
  return (
    <section id="availability" className="scroll-mt-24 px-4 py-20 sm:px-6 lg:py-28">
      <div className="mx-auto max-w-site">
        <Reveal className="max-w-2xl">
          <Eyebrow>Availability</Eyebrow>
          <h2 className="mt-4 font-display text-4xl font-semibold leading-[1.06] text-ink-950 sm:text-5xl">
            What&apos;s built, and what&apos;s next.
          </h2>
        </Reveal>
        <div className="mt-14 grid gap-4 lg:grid-cols-2">
          <Reveal className="rounded-hero bg-surface p-7 shadow-soft sm:p-10">
            <p className="flex items-center gap-2.5 font-display text-lg font-semibold text-positive">
              <LiveDot /> Built for your account
            </p>
            <ul className="mt-6 grid gap-1">
              {available.map(item => (
                <li key={item} className="flex items-center gap-3 py-2.5 text-[1.0625rem] text-ink-800">
                  <BadgeCheck aria-hidden className="size-5 shrink-0 text-positive" />
                  {item}
                </li>
              ))}
            </ul>
          </Reveal>
          <Reveal delay={100} className="rounded-hero bg-paper-2 p-7 sm:p-10">
            <p className="font-display text-lg font-semibold text-ink-600">In development</p>
            <ul className="mt-6 grid gap-1">
              {upcoming.map(item => (
                <li key={item} className="flex items-center gap-3 py-2.5 text-[1.0625rem] text-ink-500">
                  <span aria-hidden className="size-1.5 shrink-0 rounded-full bg-ink-300" />
                  {item}
                </li>
              ))}
            </ul>
          </Reveal>
        </div>
        <p className="mt-8 max-w-3xl text-[0.9375rem] leading-relaxed text-ink-500">
          Financial services launch only after identity verification, eligibility checks and the required approvals
          are in place. Creating an account does not by itself make you eligible to deposit, withdraw, convert or
          transfer digital assets or make bank payments.
        </p>
      </div>
    </section>
  );
}

export function ClosingCall({ signedIn }: { signedIn: boolean }) {
  return (
    <section className="px-3 pb-6 sm:px-5">
      <Reveal className="relative isolate mx-auto max-w-[84rem] overflow-hidden rounded-hero bg-brand-500 px-6 py-16 text-center text-white sm:px-10 lg:py-24">
        <div aria-hidden className="absolute left-1/2 top-0 -z-10 h-64 w-[44rem] -translate-x-1/2 -translate-y-1/2 rounded-full bg-white/25 blur-[100px]" />
        <h2 className="mx-auto max-w-2xl font-display text-4xl font-semibold leading-[1.05] sm:text-6xl">
          Start with your DEYPAY account.
        </h2>
        <p className="mx-auto mt-5 max-w-lg text-lg leading-relaxed text-brand-100">
          It takes a few minutes. Confirm your email, sign in, and you&apos;re ready for what comes next.
        </p>
        <div className="mt-9 flex flex-col justify-center gap-3 sm:flex-row">
          {signedIn ? (
            <Button asChild size="lg" variant="inverse">
              <Link href="/app">
                Go to your account <ArrowRight aria-hidden />
              </Link>
            </Button>
          ) : (
            <>
              <Button asChild size="lg" variant="inverse">
                <Link href="/register">
                  Create your account <ArrowRight aria-hidden />
                </Link>
              </Button>
              <Button asChild size="lg" variant="ghost" className="text-white hover:bg-white/10 hover:text-white">
                <Link href="/login">I already have an account</Link>
              </Button>
            </>
          )}
        </div>
      </Reveal>
    </section>
  );
}
