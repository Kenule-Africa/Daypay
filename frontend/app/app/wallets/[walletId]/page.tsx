import { ArrowLeft } from "lucide-react";
import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { WalletDetailView, WalletsUnavailable } from "@/components/account/wallets";
import { loadWallet, requireCustomer } from "@/lib/server/session";

export const metadata: Metadata = { title: "Wallet" };

export default async function WalletPage({ params }: { params: Promise<{ walletId: string }> }) {
  const { walletId } = await params;
  await requireCustomer("/app/wallets");
  const result = await loadWallet(walletId);
  if (result.state === "not_found") notFound();
  return (
    <>
      <Link
        href="/app/wallets"
        className="mb-6 inline-flex items-center gap-1.5 text-sm font-medium text-ink-600 transition-colors hover:text-ink-900"
      >
        <ArrowLeft aria-hidden className="size-4" /> All wallets
      </Link>
      {result.state === "ok" ? <WalletDetailView wallet={result.wallet} /> : <WalletsUnavailable />}
    </>
  );
}
