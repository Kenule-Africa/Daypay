"use client";

import { ArrowLeft, MailCheck } from "lucide-react";
import Link from "next/link";
import * as React from "react";
import { AuthHeading } from "@/components/auth/auth-shell";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/field";
import { Notice } from "@/components/ui/notice";
import { describeFailure, request } from "@/lib/api";
import { validEmail } from "@/lib/validation";

/** POST /api/v1/auth/forgot-password, answered by the isolated verifier with 202 for every valid address. */
export function ForgotPasswordForm() {
  const [email, setEmail] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [failure, setFailure] = React.useState<string | null>(null);
  const [pending, setPending] = React.useState(false);
  const [sent, setSent] = React.useState<string | null>(null);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setFailure(null);
    if (!validEmail(email)) {
      setError("Enter the email address on your account.");
      return;
    }
    setError(null);
    setPending(true);
    const address = email.trim();
    const result = await request("/api/v1/auth/forgot-password", {
      method: "POST",
      session: "omit",
      json: { email: address },
    });
    setPending(false);
    if (result.ok) setSent(address);
    else if (result.status === 400) setError("Enter the email address on your account.");
    else setFailure(describeFailure(result, "Password recovery"));
  }

  if (sent) {
    return (
      <div className="animate-rise">
        <span className="inline-flex size-14 items-center justify-center rounded-feature bg-brand-50 text-brand-600">
          <MailCheck aria-hidden className="size-7" />
        </span>
        <h1 className="mt-7 font-display text-[2rem] font-semibold leading-tight text-ink-950 sm:text-4xl">
          Check your email
        </h1>
        <p className="mt-3 text-[1.0625rem] leading-relaxed text-ink-600">
          If <span className="font-semibold text-ink-900">{sent}</span> belongs to a verified DEYPAY account that can
          sign in, we&apos;ve sent a link to reset your password. It expires after a short time and works only once.
        </p>
        <p className="mt-3 text-[0.9375rem] leading-relaxed text-ink-500">
          Requesting another link replaces the previous one. Resetting your password signs you out on every device.
        </p>
        <div className="mt-8 grid gap-3">
          <Button asChild size="lg">
            <Link href="/login">Back to sign in</Link>
          </Button>
          <Button variant="ghost" onClick={() => setSent(null)}>
            Use a different email
          </Button>
        </div>
      </div>
    );
  }

  return (
    <>
      <AuthHeading
        title="Reset your password"
        description="Enter the email address on your account and we'll send you a secure reset link."
      />
      <form noValidate onSubmit={submit} className="mt-8 grid animate-rise gap-5 [animation-delay:80ms]">
        <Field label="Email address" error={error}>
          {({ id, describedBy, invalid }) => (
            <Input
              id={id}
              type="email"
              inputMode="email"
              autoComplete="email"
              autoCapitalize="none"
              spellCheck={false}
              value={email}
              onChange={event => setEmail(event.target.value)}
              aria-describedby={describedBy}
              invalid={invalid}
              disabled={pending}
            />
          )}
        </Field>
        {failure && <Notice tone="error">{failure}</Notice>}
        <Button type="submit" size="lg" loading={pending}>
          Send reset link
        </Button>
      </form>
      <Link
        href="/login"
        className="mx-auto mt-8 inline-flex items-center gap-2 rounded-md text-[0.9375rem] font-medium text-ink-600 hover:text-ink-900"
      >
        <ArrowLeft aria-hidden className="size-4" /> Back to sign in
      </Link>
    </>
  );
}
