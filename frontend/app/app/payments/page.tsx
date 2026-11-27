import { Send } from "lucide-react";
import type { Metadata } from "next";
import Link from "next/link";
import { PageHeader } from "@/components/account/page-header";
import { PaymentsUnavailable, TransactionList } from "@/components/account/payments";
import { loadTransactions, requireCustomer } from "@/lib/server/session";

export const metadata: Metadata = { title: "Payments" };

export default async function PaymentsPage({ searchParams }: { searchParams: Promise<{ cursor?: string | string[] }> }) {
  await requireCustomer("/app/payments");
  const { cursor } = await searchParams;
  const history = await loadTransactions(typeof cursor === "string" ? cursor : undefined);
  return (
    <>
      <PageHeader
        eyebrow="Payments"
        title="Send and receive"
        description="Payments between DEYPAY users move instantly on DEYPAY's ledger. You and the other customer see the same reference."
      />
      <div className="mb-6">
        <Link
          href="/app/payments/send"
          className="inline-flex h-11 items-center gap-2 rounded-control bg-brand-500 px-5 text-[0.9375rem] font-medium text-white shadow-glow transition-colors hover:bg-brand-600"
        >
          <Send aria-hidden className="size-[1.125rem]" /> Send to a DEYPAY user
        </Link>
      </div>
      {history.state === "ok" ? (
        <TransactionList transactions={history.transactions} next={history.next} />
      ) : (
        <PaymentsUnavailable />
      )}
    </>
  );
}
