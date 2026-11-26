import type { Metadata } from "next";
import { Overview } from "@/components/account/overview";
import { requireCustomer } from "@/lib/server/session";

export const metadata: Metadata = { title: "Overview" };

export default async function AccountOverviewPage() {
  await requireCustomer("/app");
  return <Overview />;
}
