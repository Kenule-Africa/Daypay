import type { Metadata } from "next";
import { redirect } from "next/navigation";
import { AuthShell } from "@/components/auth/auth-shell";
import { LoginForm } from "@/components/auth/login-form";
import { loadCustomer } from "@/lib/server/session";
import { safeNextPath } from "@/lib/validation";

export const metadata: Metadata = { title: "Sign in" };

type Search = Promise<Record<string, string | string[] | undefined>>;

export default async function LoginPage({ searchParams }: { searchParams: Search }) {
  const params = await searchParams;
  const next = safeNextPath(typeof params.next === "string" ? params.next : null);
  const customer = await loadCustomer();
  if (customer.state === "ok") redirect(next);
  const reason =
    params.reason === "expired" || params.reason === "signed_out" || params.reason === "reset"
      ? params.reason
      : null;
  return (
    <AuthShell>
      <LoginForm next={next} reason={reason} canResume={reason === "expired" && customer.state === "expired"} />
    </AuthShell>
  );
}
