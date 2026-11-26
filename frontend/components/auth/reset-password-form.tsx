"use client";

import { CircleCheckBig, KeyRound, ShieldAlert } from "lucide-react";
import Link from "next/link";
import * as React from "react";
import { AuthHeading } from "@/components/auth/auth-shell";
import { PasswordRules } from "@/components/auth/password-rules";
import { useFragmentToken } from "@/components/auth/use-fragment-token";
import { StateIcon } from "@/components/auth/verify-email";
import { Button } from "@/components/ui/button";
import { Field, PasswordInput } from "@/components/ui/field";
import { Notice } from "@/components/ui/notice";
import { Spinner } from "@/components/ui/spinner";
import { describeFailure, request } from "@/lib/api";
import { passwordAccepted } from "@/lib/validation";

/**
 * POST /api/v1/auth/reset-password with the link token and new password. The
 * ingress routes it to the isolated verifier; the request never carries the
 * session cookie, matching the backend's own reset page.
 */
export function ResetPasswordForm() {
  const token = useFragmentToken();
  const [password, setPassword] = React.useState("");
  const [confirmation, setConfirmation] = React.useState("");
  const [errors, setErrors] = React.useState<{ password?: string; confirmation?: string }>({});
  const [failure, setFailure] = React.useState<string | null>(null);
  const [pending, setPending] = React.useState(false);
  const [outcome, setOutcome] = React.useState<"form" | "done" | "invalid">("form");

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!token) return;
    const found: typeof errors = {};
    if (!passwordAccepted(password)) found.password = "Your new password doesn't meet every requirement yet.";
    if (password !== confirmation) found.confirmation = "The passwords don't match.";
    setErrors(found);
    setFailure(null);
    if (found.password || found.confirmation) return;
    setPending(true);
    const result = await request("/api/v1/auth/reset-password", {
      method: "POST",
      session: "omit",
      json: { token, password },
    });
    setPending(false);
    setPassword("");
    setConfirmation("");
    if (result.ok) setOutcome("done");
    else if (result.status === 422 && result.reason === "password_rejected")
      setErrors({
        password:
          "That password doesn't meet the requirements or has appeared in a known data breach. Choose a different one.",
      });
    else if (result.status === 422) setOutcome("invalid");
    else if (result.status === 429) setFailure("Too many attempts. Wait a minute, then try again.");
    else setFailure(describeFailure(result, "Password reset"));
  }

  if (token === undefined) return <Spinner label="Loading" className="text-ink-500" />;

  if (outcome === "done") {
    return (
      <div className="animate-rise">
        <StateIcon tone="positive">
          <CircleCheckBig aria-hidden className="size-7" />
        </StateIcon>
        <AuthHeading
          title="Password updated"
          description="Your password has been reset and every session has been signed out. Sign in with your new password."
        />
        <Button asChild size="lg" className="mt-8 w-full">
          <Link href="/login?reason=reset">Sign in</Link>
        </Button>
      </div>
    );
  }

  if (!token || outcome === "invalid") {
    return (
      <div className="animate-rise">
        <StateIcon tone={token ? "danger" : "brand"}>
          {token ? <ShieldAlert aria-hidden className="size-7" /> : <KeyRound aria-hidden className="size-7" />}
        </StateIcon>
        <AuthHeading
          title={token ? "This reset link can't be used" : "Open your reset link"}
          description={
            token
              ? "The link is invalid, has expired or was already used. Request a new password reset email."
              : "Open the password reset link from your email on this device."
          }
        />
        <Button asChild size="lg" className="mt-8 w-full">
          <Link href="/forgot-password">Request a new link</Link>
        </Button>
      </div>
    );
  }

  return (
    <>
      <AuthHeading
        title="Choose a new password"
        description="Resetting your password signs you out on every device, including this one."
      />
      <form noValidate onSubmit={submit} className="mt-8 grid animate-rise gap-5 [animation-delay:80ms]">
        <Field label="New password" error={errors.password} hint={<PasswordRules password={password} />}>
          {({ id, describedBy, invalid }) => (
            <PasswordInput
              id={id}
              autoComplete="new-password"
              value={password}
              onChange={event => setPassword(event.target.value)}
              aria-describedby={describedBy}
              invalid={invalid}
              disabled={pending}
            />
          )}
        </Field>
        <Field label="Confirm new password" error={errors.confirmation}>
          {({ id, describedBy, invalid }) => (
            <PasswordInput
              id={id}
              autoComplete="new-password"
              value={confirmation}
              onChange={event => setConfirmation(event.target.value)}
              aria-describedby={describedBy}
              invalid={invalid}
              disabled={pending}
            />
          )}
        </Field>
        {failure && <Notice tone="error">{failure}</Notice>}
        <Button type="submit" size="lg" loading={pending}>
          Reset password
        </Button>
      </form>
    </>
  );
}
