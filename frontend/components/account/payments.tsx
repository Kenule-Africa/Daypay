import { ArrowDownLeft, ArrowRight, ArrowUpRight, ServerCrash } from "lucide-react";
import Link from "next/link";
import type { ReactNode } from "react";
import type { Transaction } from "@/lib/contracts";
import { counterpartyName, transactionStatus } from "@/lib/transfers";
import { cn } from "@/lib/utils";
import { displayAmount, type Tone } from "@/lib/wallets";

const toneClass: Record<Tone, string> = {
  positive: "bg-positive-soft text-positive",
  caution: "bg-caution-soft text-caution",
  danger: "bg-danger-soft text-danger",
  neutral: "bg-paper-2 text-ink-600",
};

function StatusPill({ transaction }: { transaction: Transaction }) {
  const status = transactionStatus[transaction.status] ?? { label: "Unknown", tone: "neutral" as Tone };
  return (
    <span className={cn("inline-flex w-fit items-center rounded-full px-2.5 py-1 text-xs font-semibold", toneClass[status.tone])}>
      {status.label}
    </span>
  );
}

/** An exact ledger amount with its direction; an unreadable one is never replaced. */
function SignedAmount({ transaction, className }: { transaction: Transaction; className?: string }) {
  const shown = displayAmount(transaction.source_amount);
  const sign = transaction.direction === "debit" ? "−" : "+";
  return (
    <span
      className={cn("tabular-nums", transaction.direction === "credit" ? "text-positive" : "text-ink-950", className)}
    >
      {shown ? `${sign}${shown}` : "—"} <span className="text-[0.7em] font-medium text-ink-500">{transaction.source_asset}</span>
    </span>
  );
}

function when(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleString("en-NG", { dateStyle: "medium", timeStyle: "short", timeZone: "Africa/Lagos" });
}

function headline(transaction: Transaction): string {
  const who = counterpartyName(transaction.counterparty);
  return transaction.direction === "debit" ? `Sent to ${who}` : `Received from ${who}`;
}

export function PaymentsUnavailable() {
  return (
    <section className="rounded-feature bg-surface p-6 shadow-soft sm:p-8">
      <span className="inline-flex size-12 items-center justify-center rounded-feature bg-caution-soft text-caution">
        <ServerCrash aria-hidden className="size-6" />
      </span>
      <h2 className="mt-5 font-display text-xl font-semibold text-ink-950">We can&apos;t load your payments right now</h2>
      <p className="mt-2 max-w-xl text-[0.9375rem] leading-relaxed text-ink-600">
        Your payment history comes from DEYPAY&apos;s records, and they aren&apos;t responding. Nothing is shown rather
        than an incomplete history. Please try again in a moment.
      </p>
    </section>
  );
}

export function TransactionList({ transactions, next }: { transactions: Transaction[]; next: string | null }) {
  if (transactions.length === 0) {
    return (
      <section className="rounded-feature bg-surface p-6 shadow-soft sm:p-8">
        <h2 className="font-display text-xl font-semibold text-ink-950">No payments yet</h2>
        <p className="mt-2 max-w-xl text-[0.9375rem] leading-relaxed text-ink-600">
          Money you send to or receive from other DEYPAY users will appear here, with the same reference both of you
          see.
        </p>
      </section>
    );
  }
  return (
    <section className="rounded-feature bg-surface p-2 shadow-soft sm:p-3">
      <ul className="grid">
        {transactions.map(transaction => (
          <li key={transaction.id}>
            <Link
              href={`/app/payments/${transaction.id}`}
              className="group flex items-center gap-4 rounded-panel p-3 transition-colors duration-200 hover:bg-paper sm:p-4"
            >
              <span
                aria-hidden
                className={cn(
                  "inline-flex size-10 shrink-0 items-center justify-center rounded-full",
                  transaction.direction === "credit" ? "bg-positive-soft text-positive" : "bg-paper-2 text-ink-700",
                )}
              >
                {transaction.direction === "credit" ? (
                  <ArrowDownLeft className="size-5" />
                ) : (
                  <ArrowUpRight className="size-5" />
                )}
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate font-medium text-ink-900">{headline(transaction)}</span>
                <span className="block truncate text-sm text-ink-500">
                  {when(transaction.created_at)} · {transaction.reference}
                </span>
              </span>
              <span className="flex flex-col items-end gap-1">
                <SignedAmount transaction={transaction} className="font-medium" />
                <StatusPill transaction={transaction} />
              </span>
            </Link>
          </li>
        ))}
      </ul>
      {next && (
        <div className="flex justify-center p-3">
          <Link
            href={`/app/payments?cursor=${next}`}
            className="inline-flex items-center gap-1.5 text-sm font-medium text-brand-600 hover:underline"
          >
            Older payments <ArrowRight aria-hidden className="size-4" />
          </Link>
        </div>
      )}
    </section>
  );
}

export function TransactionDetailView({ transaction }: { transaction: Transaction }) {
  const rows: [string, ReactNode][] = [
    [transaction.direction === "debit" ? "To" : "From", counterpartyName(transaction.counterparty)],
    ["Reference", <span key="reference" className="font-mono">{transaction.reference}</span>],
    ["Asset", transaction.source_asset],
    ["Fee", `${displayAmount(transaction.fee) ?? "—"} ${transaction.source_asset}`],
    ["Note", transaction.description ?? "—"],
    ["Created", when(transaction.created_at)],
    ["Completed", transaction.completed_at ? when(transaction.completed_at) : "—"],
  ];
  return (
    <section className="rounded-feature bg-surface p-6 shadow-soft sm:p-8">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="text-sm text-ink-500">{headline(transaction)}</p>
          <SignedAmount transaction={transaction} className="mt-1 block font-display text-3xl font-semibold" />
        </div>
        <StatusPill transaction={transaction} />
      </div>
      <dl className="mt-6 grid gap-3 rounded-panel bg-paper p-4 text-sm sm:grid-cols-2">
        {rows.map(([label, value]) => (
          <div key={label} className="min-w-0">
            <dt className="text-ink-500">{label}</dt>
            <dd className="mt-0.5 break-words font-medium text-ink-900">{value}</dd>
          </div>
        ))}
      </dl>
      <p className="mt-4 text-sm text-ink-500">
        Both you and the other customer see this payment under the same reference.
      </p>
    </section>
  );
}
