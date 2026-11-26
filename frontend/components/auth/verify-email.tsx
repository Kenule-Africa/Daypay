"use client";

import { CircleCheckBig, MailQuestion, ShieldAlert } from "lucide-react";
import Link from "next/link";
import * as React from "react";
import { AuthHeading } from "@/components/auth/auth-shell";
import { ResendVerification } from "@/components/auth/resend-verification";
import { useFragmentToken } from "@/components/auth/use-fragment-token";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/field";
import { Notice } from "@/components/ui/notice";
import { Spinner } from "@/components/ui/spinner";
import { describeFailure, request } from "@/lib/api";
import { validEmail } from "@/lib/validation";

type Outcome = "ready" | "verifying" | "verified" | "invalid";

export function VerifyEmail() {
  const token = useFragmentToken();
  const [outcome, setOutcome] = React.useState<Outcome>("ready");
  const [failure, setFailure] = React.useState<string | null>(null);

  async function verify() {
    if (!token) return;
    setOutcome("verifying");
    setFailure(null);
    const result = await request("/api/v1/auth/verify-email", {
      method: "POST",
      session: "omit",
      json: { token },
    });
    if (result.ok) setOutcome("verified");
    else if (result.status === 422 || result.status === 400) setOutcome("invalid");
    else {
      setOutcome("ready");
      setFailure(describeFailure(result, "Verification"));
    }
  }

  if (token === undefined) {
    return <Spinner label="Loading" className="text-ink-500" />;
  }

  if (outcome === "verified") {
    return (
      <div className="animate-rise">
        <StateIcon tone="positive">
          <CircleCheckBig aria-hidden className="size-7" />
        </StateIcon>
        <AuthHeading
          title="Your email is verified"
          description="Thanks for confirming. You can now sign in to your DEYPAY account."
        />
        <Button asChild size="lg" className="mt-8 w-full">
          <Link href="/login">Sign in</Link>
        </Button>
      </div>
    );
  }

  if (!token || outcome === "invalid") {
    return (
      <div className="animate-rise">
        <StateIcon tone={token ? "danger" : "brand"}>
          {token ? <ShieldAlert aria-hidden className="size-7" /> : <MailQuestion aria-hidden className="size-7" />}
        </StateIcon>
        <AuthHeading
          title={token ? "This link can't be used" : "Verify your email"}
          description={
            token
              ? "The verification link is invalid, has expired or was already used. Request a new one below."
              : "Open the verification link from your email on this device. Need a new link? Enter your email below."
          }
        />
        <RequestNewLink />
      </div>
    );
  }

  return (
    <div className="animate-rise">
      <StateIcon tone="brand">
        <MailQuestion aria-hidden className="size-7" />
      </StateIcon>
      <AuthHeading title="Confirm your email" description="Confirm that this email address belongs to you." />
      {failure && (
        <Notice tone="error" className="mt-6">
          {failure}
        </Notice>
      )}
      <Button size="lg" className="mt-8 w-full" onClick={verify} loading={outcome === "verifying"}>
        Confirm my email
      </Button>
    </div>
  );
}

function RequestNewLink() {
  const [email, setEmail] = React.useState("");
  const [confirmed, setConfirmed] = React.useState<string | null>(null);
  const [error, setError] = React.useState<string | null>(null);

  if (confirmed) {
    return (
      <div className="mt-8">
        <p className="mb-3 text-[0.9375rem] text-ink-600">
          Sending to <span className="font-semibold text-ink-900">{confirmed}</span>
        </p>
        <ResendVerification email={confirmed} />
      </div>
    );
  }

  return (
    <form
      noValidate
      className="mt-8 grid gap-4"
      onSubmit={event => {
        event.preventDefault();
        if (!validEmail(email)) {
          setError("Enter the email address you registered with.");
          return;
        }
        setConfirmed(email.trim());
      }}
    >
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
            onChange={event => {
              setEmail(event.target.value);
              setError(null);
            }}
            aria-describedby={describedBy}
            invalid={invalid}
          />
        )}
      </Field>
      <Button type="submit" size="lg" variant="ink">
        Continue
      </Button>
    </form>
  );
}

export function StateIcon({ tone, children }: { tone: "brand" | "positive" | "danger"; children: React.ReactNode }) {
  const classes = {
    brand: "bg-brand-50 text-brand-600",
    positive: "bg-positive-soft text-positive",
    danger: "bg-danger-soft text-danger",
  };
  return (
    <span className={`mb-7 inline-flex size-14 items-center justify-center rounded-feature ${classes[tone]}`}>{children}</span>
  );
}
