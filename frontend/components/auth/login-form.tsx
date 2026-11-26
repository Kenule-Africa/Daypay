"use client";

import { ArrowRight } from "lucide-react";
import Link from "next/link";
import * as React from "react";
import { AuthHeading } from "@/components/auth/auth-shell";
import { Button } from "@/components/ui/button";
import { Field, Input, PasswordInput } from "@/components/ui/field";
import { Notice } from "@/components/ui/notice";
import { Spinner } from "@/components/ui/spinner";
import { describeFailure, refreshSession, request } from "@/lib/api";
import { validEmail } from "@/lib/validation";

type Reason = "expired" | "signed_out" | "reset" | null;

const reasons: Record<Exclude<Reason, null>, { tone: "info" | "success"; text: string }> = {
  expired: { tone: "info", text: "Your session has ended. Sign in again to continue." },
  signed_out: { tone: "success", text: "You've signed out of DEYPAY." },
  reset: { tone: "success", text: "Your password has been reset. Sign in with your new password." },
};

export function LoginForm({ next, reason, canResume }: { next: string; reason: Reason; canResume: boolean }) {
  const [email, setEmail] = React.useState("");
  const [password, setPassword] = React.useState("");
  const [errors, setErrors] = React.useState<{ email?: string; password?: string }>({});
  const [failure, setFailure] = React.useState<React.ReactNode>(null);
  const [pending, setPending] = React.useState(false);
  const [resuming, setResuming] = React.useState(canResume);

  // A session that the account area rejected may only need rotation, for
  // example after an account status change. Try one refresh before asking for a password.
  React.useEffect(() => {
    if (!canResume) return;
    // Never retry within a minute, so a refresh the account area still rejects cannot loop.
    const key = "deypay:resume-attempt";
    const last = Number(window.sessionStorage.getItem(key) ?? 0);
    if (Date.now() - last < 60_000) {
      setResuming(false);
      return;
    }
    window.sessionStorage.setItem(key, String(Date.now()));
    let active = true;
    refreshSession().then(ok => {
      if (!active) return;
      if (ok) window.location.replace(next);
      else setResuming(false);
    });
    return () => {
      active = false;
    };
  }, [canResume, next]);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    const found: typeof errors = {};
    if (!validEmail(email)) found.email = "Enter the email address you registered with.";
    if (!password) found.password = "Enter your password.";
    setErrors(found);
    setFailure(null);
    if (found.email || found.password) return;
    setPending(true);
    const result = await request("/api/v1/auth/login", {
      method: "POST",
      session: "optional",
      json: { identifier: email.trim(), password },
    });
    if (result.ok) {
      window.location.replace(next);
      return;
    }
    setPending(false);
    setPassword("");
    if (result.status === 400) {
      setErrors({ email: "Enter the email address you registered with." });
    } else if (result.status === 401) {
      setFailure(
        <>
          That email and password don&apos;t match an account that can sign in. If you registered recently,{" "}
          <Link href="/verify-email" className="font-medium underline underline-offset-2">
            confirm your email
          </Link>{" "}
          first.
        </>,
      );
    } else {
      setFailure(describeFailure(result, "Sign-in"));
    }
  }

  if (resuming) {
    return (
      <div className="flex animate-fade items-center gap-3 text-ink-600">
        <Spinner label="Restoring your session" />
        <span aria-hidden>Restoring your session…</span>
      </div>
    );
  }

  const notice = reason ? reasons[reason] : null;

  return (
    <>
      <AuthHeading title="Welcome back" description="Sign in to your DEYPAY account." />
      {notice && (
        <Notice tone={notice.tone} className="mt-7">
          {notice.text}
        </Notice>
      )}
      <form noValidate onSubmit={submit} className="mt-8 grid animate-rise gap-5 [animation-delay:80ms]">
        <Field label="Email address" error={errors.email}>
          {({ id, describedBy, invalid }) => (
            <Input
              id={id}
              type="email"
              inputMode="email"
              autoComplete="username"
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
        <Field label="Password" error={errors.password}>
          {({ id, describedBy, invalid }) => (
            <PasswordInput
              id={id}
              autoComplete="current-password"
              value={password}
              onChange={event => setPassword(event.target.value)}
              aria-describedby={describedBy}
              invalid={invalid}
              disabled={pending}
            />
          )}
        </Field>
        <div className="-mt-1 flex justify-end">
          <Link href="/forgot-password" className="rounded-md text-sm font-medium text-brand-600 hover:text-brand-700">
            Forgot password?
          </Link>
        </div>
        {failure && <Notice tone="error">{failure}</Notice>}
        <Button type="submit" size="lg" loading={pending}>
          Sign in <ArrowRight aria-hidden />
        </Button>
      </form>
      <p className="mt-8 text-center text-[0.9375rem] text-ink-500">
        New to DEYPAY?{" "}
        <Link href="/register" className="rounded-md font-medium text-brand-600 hover:text-brand-700">
          Create an account
        </Link>
      </p>
    </>
  );
}
