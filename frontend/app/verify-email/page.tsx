import type { Metadata } from "next";
import { AuthShell } from "@/components/auth/auth-shell";
import { VerifyEmail } from "@/components/auth/verify-email";

export const metadata: Metadata = { title: "Verify your email", referrer: "no-referrer" };

export default function VerifyEmailPage() {
  return (
    <AuthShell>
      <VerifyEmail />
    </AuthShell>
  );
}
