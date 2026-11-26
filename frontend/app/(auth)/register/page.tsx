import type { Metadata } from "next";
import { redirect } from "next/navigation";
import { AuthShell } from "@/components/auth/auth-shell";
import { RegisterForm } from "@/components/auth/register-form";
import { PRIVACY_ID, TERMS_ID } from "@/lib/contracts";
import { countries } from "@/lib/countries";
import { legalIndex } from "@/lib/server/legal";
import { loadCustomer } from "@/lib/server/session";

export const metadata: Metadata = { title: "Create your account" };

export default async function RegisterPage() {
  const [customer, index] = await Promise.all([loadCustomer(), legalIndex()]);
  if (customer.state === "ok") redirect("/app");
  const terms = index?.documents.find(document => document.document_id === TERMS_ID);
  const privacy = index?.documents.find(document => document.document_id === PRIVACY_ID);
  return (
    <AuthShell>
      <RegisterForm countries={countries()} legal={terms && privacy ? { terms, privacy } : null} />
    </AuthShell>
  );
}
