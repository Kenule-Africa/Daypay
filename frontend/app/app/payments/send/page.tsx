import { ArrowLeft } from "lucide-react";
import type { Metadata } from "next";
import Link from "next/link";
import { PageHeader } from "@/components/account/page-header";
import { SendTransfer } from "@/components/account/send-transfer";
import { WalletsUnavailable } from "@/components/account/wallets";
import { Notice } from "@/components/ui/notice";
import { loadWallets, requireCustomer } from "@/lib/server/session";

export const metadata: Metadata = { title: "Send" };

export default async function SendPage({ searchParams }: { searchParams: Promise<{ asset?: string | string[] }> }) {
  await requireCustomer("/app/payments/send");
  const wallets = await loadWallets();
  const { asset } = await searchParams;
  const sendable = wallets.state === "ok" ? wallets.wallets.filter(wallet => wallet.capabilities.send && wallet.balance) : [];
  const canStart = asset === undefined ? sendable.length > 0 : sendable.some(wallet => wallet.asset === asset);
  return (
    <>
      <Link
        href="/app/payments"
        className="mb-6 inline-flex items-center gap-1.5 text-sm font-medium text-ink-600 transition-colors hover:text-ink-900"
      >
        <ArrowLeft aria-hidden className="size-4" /> All payments
      </Link>
      <PageHeader
        eyebrow="Payments"
        title="Send to a DEYPAY user"
        description="Enter their exact DEYPAY username. You'll see who they are and the full terms before you confirm."
      />
      {wallets.state !== "ok" ? (
        <WalletsUnavailable />
      ) : canStart ? (
        <SendTransfer wallets={sendable} initialAsset={typeof asset === "string" ? asset : undefined} />
      ) : (
        <Notice tone="info" title="Sending isn't open yet">
          Sending money on DEYPAY opens once your account and wallet are eligible and DEYPAY has completed its
          approvals. Your balance stays safe in the meantime.
        </Notice>
      )}
    </>
  );
}
