import type { Metadata } from "next";
import { PageHeader } from "@/components/account/page-header";
import { WalletList, WalletsUnavailable } from "@/components/account/wallets";
import { loadWallets, requireCustomer } from "@/lib/server/session";

export const metadata: Metadata = { title: "Wallets" };

export default async function WalletsPage() {
  await requireCustomer("/app/wallets");
  const wallets = await loadWallets();
  return (
    <>
      <PageHeader
        eyebrow="Wallets"
        title="Your digital dollars"
        description="Balances come straight from DEYPAY's ledger. Available funds are yours to use once payments open; reserved funds are held for operations in progress."
      />
      {wallets.state === "ok" ? <WalletList wallets={wallets.wallets} /> : <WalletsUnavailable />}
    </>
  );
}
