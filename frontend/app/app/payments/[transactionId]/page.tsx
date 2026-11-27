import { ArrowLeft } from "lucide-react";
import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { PaymentsUnavailable, TransactionDetailView } from "@/components/account/payments";
import { loadTransaction, requireCustomer } from "@/lib/server/session";

export const metadata: Metadata = { title: "Payment" };

export default async function PaymentPage({ params }: { params: Promise<{ transactionId: string }> }) {
  const { transactionId } = await params;
  await requireCustomer("/app/payments");
  const result = await loadTransaction(transactionId);
  if (result.state === "not_found") notFound();
  return (
    <>
      <Link
        href="/app/payments"
        className="mb-6 inline-flex items-center gap-1.5 text-sm font-medium text-ink-600 transition-colors hover:text-ink-900"
      >
        <ArrowLeft aria-hidden className="size-4" /> All payments
      </Link>
      {result.state === "ok" ? <TransactionDetailView transaction={result.transaction} /> : <PaymentsUnavailable />}
    </>
  );
}
