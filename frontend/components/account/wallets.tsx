import { ArrowDownLeft, ArrowRight, ArrowUpRight, Clock, Info, Network, ServerCrash } from "lucide-react";
import Link from "next/link";
import { CopyAddress, RequestAddress } from "@/components/account/wallet-actions";
import { Badge } from "@/components/ui/badge";
import type { Wallet, WalletDetail, WalletNetwork } from "@/lib/contracts";
import { cn } from "@/lib/utils";
import { addressStatus, displayAmount, restrictionText, walletStatus, type Tone } from "@/lib/wallets";

const toneClass: Record<Tone, string> = {
  positive: "bg-positive-soft text-positive",
  caution: "bg-caution-soft text-caution",
  danger: "bg-danger-soft text-danger",
  neutral: "bg-paper-2 text-ink-600",
};

function StatusPill({ wallet }: { wallet: Wallet }) {
  const status = walletStatus[wallet.status];
  return (
    <span className={cn("inline-flex w-fit items-center rounded-full px-2.5 py-1 text-xs font-semibold", toneClass[status.tone])}>
      {status.label}
    </span>
  );
}

/** An exact ledger amount; an unreadable one is never replaced by zero. */
function Amount({ value, asset, className }: { value: string; asset: string; className?: string }) {
  const shown = displayAmount(value);
  return (
    <span className={cn("tabular-nums", className)}>
      {shown ?? "—"} <span className="text-[0.7em] font-medium text-ink-500">{asset}</span>
    </span>
  );
}

function Restrictions({ wallet }: { wallet: Wallet }) {
  if (wallet.restrictions.length === 0) return null;
  return (
    <ul className="grid gap-1.5">
      {wallet.restrictions.map(code => (
        <li key={code} className="flex items-start gap-2 text-sm leading-snug text-ink-600">
          <Info aria-hidden className="mt-0.5 size-4 shrink-0 text-ink-400" />
          {restrictionText[code]}
        </li>
      ))}
    </ul>
  );
}

export function WalletsUnavailable() {
  return (
    <section className="rounded-feature bg-surface p-6 shadow-soft sm:p-8">
      <span className="inline-flex size-12 items-center justify-center rounded-feature bg-caution-soft text-caution">
        <ServerCrash aria-hidden className="size-6" />
      </span>
      <h2 className="mt-5 font-display text-xl font-semibold text-ink-950">We can&apos;t load your wallets right now</h2>
      <p className="mt-2 max-w-xl text-[0.9375rem] leading-relaxed text-ink-600">
        Your balances come from DEYPAY&apos;s ledger, and it isn&apos;t responding. Nothing is shown rather than an
        out-of-date or estimated amount. Please try again in a moment.
      </p>
    </section>
  );
}

export function WalletList({ wallets }: { wallets: Wallet[] }) {
  if (wallets.length === 0) {
    return (
      <section className="rounded-feature bg-surface p-6 shadow-soft sm:p-8">
        <h2 className="font-display text-xl font-semibold text-ink-950">Your wallets aren&apos;t ready yet</h2>
        <p className="mt-2 max-w-xl text-[0.9375rem] leading-relaxed text-ink-600">
          USDC and USDT wallets are created once your account is verified. Check back after confirming your email.
        </p>
      </section>
    );
  }
  return (
    <ul className="grid gap-5 md:grid-cols-2">
      {wallets.map(wallet => (
        <li key={wallet.id}>
          <Link
            href={`/app/wallets/${wallet.id}`}
            className="group flex h-full flex-col gap-5 rounded-feature bg-surface p-5 shadow-soft transition-[box-shadow,transform] duration-300 ease-out-expo hover:-translate-y-0.5 hover:shadow-lift sm:p-7"
          >
            <div className="flex items-start justify-between gap-3">
              <div>
                <p className="font-display text-xl font-semibold text-ink-950">{wallet.asset}</p>
                <p className="text-sm text-ink-500">{wallet.asset_name}</p>
              </div>
              <StatusPill wallet={wallet} />
            </div>
            {wallet.balance ? (
              <div>
                <p className="text-sm text-ink-500">Total balance</p>
                <Amount
                  value={wallet.balance.total}
                  asset={wallet.asset}
                  className="mt-1 block font-display text-3xl font-semibold text-ink-950"
                />
                <dl className="mt-4 grid grid-cols-2 gap-3 rounded-panel bg-paper p-3.5 text-sm">
                  <div>
                    <dt className="text-ink-500">Available</dt>
                    <dd className="mt-0.5 font-medium text-ink-900">
                      <Amount value={wallet.balance.available} asset={wallet.asset} />
                    </dd>
                  </div>
                  <div>
                    <dt className="text-ink-500">Reserved</dt>
                    <dd className="mt-0.5 font-medium text-ink-900">
                      <Amount value={wallet.balance.reserved} asset={wallet.asset} />
                    </dd>
                  </div>
                </dl>
              </div>
            ) : (
              <p className="text-[0.9375rem] text-ink-600">This wallet is still being set up.</p>
            )}
            <Restrictions wallet={wallet} />
            <span className="mt-auto inline-flex items-center gap-1.5 text-sm font-medium text-brand-600">
              Wallet details
              <ArrowRight aria-hidden className="size-4 transition-transform duration-300 group-hover:translate-x-0.5" />
            </span>
          </Link>
        </li>
      ))}
    </ul>
  );
}

function NetworkRow({ wallet, network }: { wallet: WalletDetail; network: WalletNetwork }) {
  const { deposit } = network;
  return (
    <li className="grid gap-3 rounded-panel bg-paper p-4 sm:grid-cols-[1fr_auto] sm:items-start">
      <div className="min-w-0">
        <p className="font-medium text-ink-900">{network.name}</p>
        <p className="text-sm text-ink-500">
          {deposit.available ? "Deposits available" : "Deposits not available"}
          {network.required_confirmations ? ` · ${network.required_confirmations} confirmations` : ""}
        </p>
        {deposit.address_status === "active" && deposit.address ? (
          <div className="mt-3 grid gap-2">
            <p className="break-all rounded-control bg-surface px-3 py-2 font-mono text-sm text-ink-900">
              {deposit.address.address}
            </p>
            {deposit.address.memo && (
              <p className="text-sm text-ink-700">
                Memo: <span className="font-mono">{deposit.address.memo}</span>
              </p>
            )}
            <p className="text-xs leading-snug text-ink-500">
              {`Send only ${wallet.asset} on ${network.name} to this address. Other assets or networks may be lost.`}
            </p>
          </div>
        ) : null}
      </div>
      <div className="flex flex-wrap items-center gap-2 sm:justify-end">
        {deposit.address_status === "active" && deposit.address ? (
          <CopyAddress address={deposit.address.address} />
        ) : deposit.address_status === "not_requested" && deposit.available ? (
          <RequestAddress walletId={wallet.id} network={network.network} />
        ) : (
          <Badge tone={deposit.address_status === "pending" ? "caution" : "soon"}>
            <Clock aria-hidden /> {addressStatus[deposit.address_status]}
          </Badge>
        )}
      </div>
    </li>
  );
}

export function WalletDetailView({ wallet }: { wallet: WalletDetail }) {
  return (
    <div className="grid gap-5 lg:gap-6">
      <section className="relative isolate overflow-hidden rounded-hero bg-ink-950 p-6 text-paper shadow-pop sm:p-9">
        <div aria-hidden className="absolute -right-20 -top-24 -z-10 size-72 rounded-full bg-brand-600/30 blur-[80px]" />
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="font-display text-lg font-semibold text-white">{`${wallet.asset} wallet`}</p>
          <StatusPill wallet={wallet} />
        </div>
        {wallet.balance ? (
          <>
            <p className="mt-6 text-sm text-ink-300">Total balance</p>
            <p className="mt-1 font-display text-4xl font-semibold text-white sm:text-5xl">
              <Amount value={wallet.balance.total} asset={wallet.asset} />
            </p>
            <dl className="mt-6 grid grid-cols-2 gap-3 rounded-panel bg-white/[0.06] p-4 text-sm">
              <div>
                <dt className="text-ink-300">Available</dt>
                <dd className="mt-0.5 font-medium text-white">
                  <Amount value={wallet.balance.available} asset={wallet.asset} />
                </dd>
              </div>
              <div>
                <dt className="text-ink-300">Reserved</dt>
                <dd className="mt-0.5 font-medium text-white">
                  <Amount value={wallet.balance.reserved} asset={wallet.asset} />
                </dd>
              </div>
            </dl>
          </>
        ) : (
          <p className="mt-6 text-ink-300">This wallet is still being set up.</p>
        )}
        <div className="mt-6 flex flex-wrap gap-2">
          {[
            { icon: ArrowDownLeft, label: "Deposit" },
            { icon: ArrowUpRight, label: "Send" },
          ].map(({ icon: Icon, label }) => (
            <span
              key={label}
              aria-disabled="true"
              className="inline-flex items-center gap-2 rounded-control bg-white/[0.07] px-3.5 py-2 text-sm text-ink-300"
            >
              <Icon aria-hidden className="size-4" />
              {label}
              <Badge tone="dark">Not yet available</Badge>
            </span>
          ))}
        </div>
      </section>

      {wallet.restrictions.length > 0 && (
        <section className="rounded-feature bg-surface p-5 shadow-soft sm:p-7">
          <h2 className="font-display text-lg font-semibold text-ink-900">What&apos;s limited right now</h2>
          <div className="mt-4">
            <Restrictions wallet={wallet} />
          </div>
        </section>
      )}

      <section aria-labelledby="networks-title" className="rounded-feature bg-surface p-5 shadow-soft sm:p-7">
        <div className="flex items-center gap-2.5">
          <Network aria-hidden className="size-5 text-brand-500" />
          <h2 id="networks-title" className="font-display text-lg font-semibold text-ink-900">
            Deposit networks
          </h2>
        </div>
        {wallet.networks.length === 0 ? (
          <p className="mt-3 max-w-2xl text-[0.9375rem] leading-relaxed text-ink-600">
            {`No network is available for ${wallet.asset} deposits yet. Supported networks appear here once DEYPAY has confirmed them, and a deposit address is only shown after it has been verified.`}
          </p>
        ) : (
          <ul className="mt-4 grid gap-3">
            {wallet.networks.map(network => (
              <NetworkRow key={network.network} wallet={wallet} network={network} />
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
