import type { Metadata } from "next";
import { SecurityCenter } from "@/components/account/security-center";
import { requireCustomer } from "@/lib/server/session";

export const metadata: Metadata = { title: "Security" };

export default async function SecurityPage() {
  await requireCustomer("/app/security");
  return <SecurityCenter />;
}
