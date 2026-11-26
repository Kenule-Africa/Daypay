"use client";

import { ArrowLeft, ArrowRight, AtSign, Check, CircleAlert, MailCheck } from "lucide-react";
import Link from "next/link";
import * as React from "react";
import { AuthHeading } from "@/components/auth/auth-shell";
import { PasswordRules } from "@/components/auth/password-rules";
import { ResendVerification } from "@/components/auth/resend-verification";
import { useUsernameAvailability, type Availability } from "@/components/auth/use-username-availability";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Field, Input, PasswordInput, inputClass } from "@/components/ui/field";
import { Notice } from "@/components/ui/notice";
import { Spinner } from "@/components/ui/spinner";
import { describeFailure, request } from "@/lib/api";
import type { LegalDocument } from "@/lib/contracts";
import type { Country } from "@/lib/countries";
import { cn } from "@/lib/utils";
import {
  normalizePhone,
  passwordAccepted,
  usernameHandle,
  validEmail,
  validName,
  validPhone,
} from "@/lib/validation";

type Legal = { terms: LegalDocument; privacy: LegalDocument };

type Form = {
  first_name: string;
  last_name: string;
  country_code: string;
  email: string;
  phone: string;
  username: string;
  password: string;
  accept_terms: boolean;
  acknowledge_privacy_notice: boolean;
};

type Errors = Partial<Record<keyof Form, string>>;

const steps = ["About you", "Contact", "Security"] as const;

const stepFields: (keyof Form)[][] = [
  ["first_name", "last_name", "country_code"],
  ["email", "phone"],
  ["username", "password", "accept_terms", "acknowledge_privacy_notice"],
];

function validate(form: Form, fields: (keyof Form)[]): Errors {
  const errors: Errors = {};
  for (const field of fields) {
    switch (field) {
      case "first_name":
        if (!validName(form.first_name)) errors.first_name = "Enter your first name as it appears on your ID.";
        break;
      case "last_name":
        if (!validName(form.last_name)) errors.last_name = "Enter your last name as it appears on your ID.";
        break;
      case "country_code":
        if (!/^[A-Z]{2}$/.test(form.country_code)) errors.country_code = "Choose your country of residence.";
        break;
      case "email":
        if (!validEmail(form.email)) errors.email = "Enter a valid email address.";
        break;
      case "phone":
        if (!validPhone(form.phone)) errors.phone = "Enter your number with its country code, for example +234 801 234 5678.";
        break;
      case "username":
        if (!usernameHandle(form.username))
          errors.username = "Use 3–30 letters, numbers, underscores or dots. It can't start or end with a dot.";
        break;
      case "password":
        if (!passwordAccepted(form.password)) errors.password = "Your password doesn't meet every requirement yet.";
        break;
      case "accept_terms":
        if (!form.accept_terms) errors.accept_terms = "You need to agree to the Terms of Service to continue.";
        break;
      case "acknowledge_privacy_notice":
        if (!form.acknowledge_privacy_notice)
          errors.acknowledge_privacy_notice = "You need to confirm you've read the Privacy Notice to continue.";
        break;
    }
  }
  return errors;
}

export function RegisterForm({ countries, legal }: { countries: Country[]; legal: Legal | null }) {
  const [step, setStep] = React.useState(0);
  const [form, setForm] = React.useState<Form>({
    first_name: "",
    last_name: "",
    country_code: "NG",
    email: "",
    phone: "",
    username: "",
    password: "",
    accept_terms: false,
    acknowledge_privacy_notice: false,
  });
  const [errors, setErrors] = React.useState<Errors>({});
  const [failure, setFailure] = React.useState<string | null>(null);
  const [pending, setPending] = React.useState(false);
  const [sentTo, setSentTo] = React.useState<string | null>(null);
  const headingRef = React.useRef<HTMLHeadingElement>(null);
  const availability = useUsernameAvailability(form.username);

  const set = <K extends keyof Form>(key: K, value: Form[K]) => {
    setForm(current => ({ ...current, [key]: value }));
    setErrors(current => ({ ...current, [key]: undefined }));
  };

  // Move focus to the new step's heading when the step changes, not on first load.
  const firstRender = React.useRef(true);
  React.useEffect(() => {
    if (firstRender.current) {
      firstRender.current = false;
      return;
    }
    headingRef.current?.focus();
  }, [step]);

  if (!legal) {
    return (
      <>
        <AuthHeading title="Create your account" />
        <Notice tone="warning" className="mt-7" title="Registration is temporarily unavailable">
          We couldn&apos;t load the current Terms of Service and Privacy Notice, which you need to review before
          signing up. Please try again shortly.
        </Notice>
      </>
    );
  }

  if (sentTo) return <RegistrationSent email={sentTo} onRestart={() => setSentTo(null)} />;

  function next(event: React.FormEvent) {
    event.preventDefault();
    const found = validate(form, stepFields[step]);
    setErrors(found);
    if (Object.keys(found).length > 0) return;
    if (step < steps.length - 1) {
      setStep(step + 1);
      return;
    }
    void submit();
  }

  async function submit() {
    if (!legal) return;
    setPending(true);
    setFailure(null);
    const email = form.email.trim();
    const result = await request("/api/v1/auth/register", {
      method: "POST",
      session: "omit",
      json: {
        first_name: form.first_name.trim(),
        last_name: form.last_name.trim(),
        country_code: form.country_code,
        email,
        phone: normalizePhone(form.phone),
        username: usernameHandle(form.username) ?? form.username,
        password: form.password,
        terms_version: legal.terms.version,
        privacy_version: legal.privacy.version,
        accept_terms: form.accept_terms,
        acknowledge_privacy_notice: form.acknowledge_privacy_notice,
      },
    });
    setPending(false);
    if (result.ok) {
      setForm(current => ({ ...current, password: "" }));
      setSentTo(email);
      return;
    }
    if (result.status === 422 || result.status === 400) {
      setFailure(
        "We couldn't accept these details. Check each step, and choose a different password if this one has appeared in a known data breach. If the legal documents were just updated, refresh the page.",
      );
    } else {
      setFailure(describeFailure(result, "Registration"));
    }
  }

  return (
    <>
      <AuthHeading title="Create your account" description="Three short steps. You'll confirm your email at the end." />
      <ol aria-label="Registration progress" className="mt-8 grid grid-cols-3 gap-2">
        {steps.map((label, index) => (
          <li key={label} aria-current={index === step ? "step" : undefined}>
            <span
              className={cn(
                "block h-1 rounded-full transition-colors duration-500 ease-out-expo",
                index <= step ? "bg-brand-500" : "bg-paper-3",
              )}
            />
            <span className={cn("mt-2 block text-xs font-medium", index === step ? "text-ink-900" : "text-ink-400")}>
              {index < step ? <Check aria-hidden className="mr-1 inline size-3 text-brand-500" /> : null}
              {label}
            </span>
          </li>
        ))}
      </ol>

      <form noValidate onSubmit={next} className="mt-8">
        <h2 ref={headingRef} tabIndex={-1} className="font-display text-xl font-semibold text-ink-900 outline-none">
          {["Tell us about yourself", "How we'll reach you", "Secure your account"][step]}
        </h2>
        <div key={step} className="mt-5 grid animate-rise gap-5">
          {step === 0 && (
            <>
              <div className="grid gap-5 sm:grid-cols-2">
                <Field label="First name" error={errors.first_name}>
                  {({ id, describedBy, invalid }) => (
                    <Input
                      id={id}
                      autoComplete="given-name"
                      value={form.first_name}
                      onChange={event => set("first_name", event.target.value)}
                      aria-describedby={describedBy}
                      invalid={invalid}
                    />
                  )}
                </Field>
                <Field label="Last name" error={errors.last_name}>
                  {({ id, describedBy, invalid }) => (
                    <Input
                      id={id}
                      autoComplete="family-name"
                      value={form.last_name}
                      onChange={event => set("last_name", event.target.value)}
                      aria-describedby={describedBy}
                      invalid={invalid}
                    />
                  )}
                </Field>
              </div>
              <Field
                label="Country of residence"
                error={errors.country_code}
                hint="Use your legal name. It will need to match your ID when identity verification opens."
              >
                {({ id, describedBy, invalid }) => (
                  <select
                    id={id}
                    autoComplete="country"
                    value={form.country_code}
                    onChange={event => set("country_code", event.target.value)}
                    aria-describedby={describedBy}
                    aria-invalid={invalid || undefined}
                    className={cn(inputClass, "appearance-none bg-[length:1rem] bg-[right_1rem_center] bg-no-repeat pr-10")}
                    style={{
                      backgroundImage:
                        "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23625d6c' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E\")",
                    }}
                  >
                    {countries.map(country => (
                      <option key={country.code} value={country.code}>
                        {country.name}
                      </option>
                    ))}
                  </select>
                )}
              </Field>
            </>
          )}

          {step === 1 && (
            <>
              <Field label="Email address" error={errors.email} hint="We'll send a link to confirm it's yours.">
                {({ id, describedBy, invalid }) => (
                  <Input
                    id={id}
                    type="email"
                    inputMode="email"
                    autoComplete="email"
                    autoCapitalize="none"
                    spellCheck={false}
                    value={form.email}
                    onChange={event => set("email", event.target.value)}
                    aria-describedby={describedBy}
                    invalid={invalid}
                  />
                )}
              </Field>
              <Field
                label="Mobile number"
                error={errors.phone}
                hint="Include your country code. Phone verification will come later."
              >
                {({ id, describedBy, invalid }) => (
                  <Input
                    id={id}
                    type="tel"
                    inputMode="tel"
                    autoComplete="tel"
                    placeholder="+234 801 234 5678"
                    value={form.phone}
                    onChange={event => set("phone", event.target.value)}
                    aria-describedby={describedBy}
                    invalid={invalid}
                  />
                )}
              </Field>
            </>
          )}

          {step === 2 && (
            <>
              <Field
                label="Username"
                error={errors.username}
                status={<AvailabilityStatus state={availability} />}
                hint="Other DEYPAY customers will be able to find you by this handle."
              >
                {({ id, describedBy, invalid }) => (
                  <div className="relative">
                    <AtSign aria-hidden className="pointer-events-none absolute left-4 top-1/2 size-4 -translate-y-1/2 text-ink-400" />
                    <Input
                      id={id}
                      autoComplete="username"
                      autoCapitalize="none"
                      spellCheck={false}
                      maxLength={31}
                      value={form.username}
                      onChange={event => set("username", event.target.value.trim())}
                      aria-describedby={describedBy}
                      invalid={invalid}
                      className="pl-10"
                    />
                  </div>
                )}
              </Field>
              <Field label="Password" error={errors.password} hint={<PasswordRules password={form.password} />}>
                {({ id, describedBy, invalid }) => (
                  <PasswordInput
                    id={id}
                    autoComplete="new-password"
                    value={form.password}
                    onChange={event => set("password", event.target.value)}
                    aria-describedby={describedBy}
                    invalid={invalid}
                  />
                )}
              </Field>
              <div className="grid gap-4 rounded-panel bg-surface p-5 shadow-soft">
                {(legal.terms.status === "sandbox_draft" || legal.privacy.status === "sandbox_draft") && (
                  <Badge tone="caution" className="w-fit">
                    Sandbox draft documents
                  </Badge>
                )}
                <Consent
                  checked={form.accept_terms}
                  onChange={value => set("accept_terms", value)}
                  error={errors.accept_terms}
                  explanation="Creating an account does not automatically make you eligible to deposit, withdraw, convert or transfer digital assets or make bank payments."
                >
                  I have read and agree to the DEYPAY{" "}
                  <LegalLink document={legal.terms}>Terms of Service</LegalLink> (Version {legal.terms.version}).
                </Consent>
                <Consent
                  checked={form.acknowledge_privacy_notice}
                  onChange={value => set("acknowledge_privacy_notice", value)}
                  error={errors.acknowledge_privacy_notice}
                  explanation="This confirms the notice was presented to you. It is not blanket consent to all processing."
                >
                  I have read and understood the DEYPAY{" "}
                  <LegalLink document={legal.privacy}>Privacy Notice</LegalLink> (Version {legal.privacy.version}).
                </Consent>
                <p className="text-[0.8125rem] leading-relaxed text-ink-500">
                  Marketing messages are optional and off by default. You can choose them later in your preferences.
                </p>
              </div>
            </>
          )}
        </div>

        {failure && (
          <Notice tone="error" className="mt-6">
            {failure}
          </Notice>
        )}

        <div className="mt-8 flex items-center gap-3">
          {step > 0 && (
            <Button type="button" variant="quiet" size="lg" onClick={() => setStep(step - 1)} disabled={pending}>
              <ArrowLeft aria-hidden /> Back
            </Button>
          )}
          <Button type="submit" size="lg" loading={pending} className="flex-1">
            {step < steps.length - 1 ? (
              <>
                Continue <ArrowRight aria-hidden />
              </>
            ) : (
              "Create account"
            )}
          </Button>
        </div>
      </form>
      <p className="mt-8 text-center text-[0.9375rem] text-ink-500">
        Already have an account?{" "}
        <Link href="/login" className="rounded-md font-medium text-brand-600 hover:text-brand-700">
          Sign in
        </Link>
      </p>
    </>
  );
}

function LegalLink({ document, children }: { document: LegalDocument; children: React.ReactNode }) {
  return (
    <Link
      href={`/legal/${encodeURIComponent(document.document_id)}/${encodeURIComponent(document.version)}`}
      target="_blank"
      className="font-medium text-brand-600 underline underline-offset-2 hover:text-brand-700"
    >
      {children}
      <span className="sr-only"> (opens in a new tab)</span>
    </Link>
  );
}

function Consent({
  checked,
  onChange,
  error,
  explanation,
  children,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  error?: string;
  explanation: string;
  children: React.ReactNode;
}) {
  const id = React.useId();
  return (
    <div>
      <div className="flex gap-3">
        <Checkbox
          id={id}
          checked={checked}
          onCheckedChange={value => onChange(value === true)}
          aria-invalid={error ? true : undefined}
          aria-describedby={`${id}-explanation${error ? ` ${id}-error` : ""}`}
        />
        <label htmlFor={id} className="text-[0.9375rem] leading-snug text-ink-800">
          {children}
        </label>
      </div>
      <p id={`${id}-explanation`} className="mt-1.5 pl-8 text-[0.8125rem] leading-relaxed text-ink-500">
        {explanation}
      </p>
      {error && (
        <p id={`${id}-error`} className="mt-1.5 animate-fade pl-8 text-sm font-medium text-danger">
          {error}
        </p>
      )}
    </div>
  );
}

function AvailabilityStatus({ state }: { state: Availability }) {
  switch (state) {
    case "checking":
      return (
        <span className="flex items-center gap-2 text-ink-500">
          <Spinner /> Checking availability…
        </span>
      );
    case "available":
      return (
        <span className="flex items-center gap-1.5 font-medium text-positive">
          <Check aria-hidden className="size-4" /> Available right now
        </span>
      );
    case "taken":
      return (
        <span className="flex items-center gap-1.5 font-medium text-danger">
          <CircleAlert aria-hidden className="size-4" /> Not available. Try another username.
        </span>
      );
    case "limited":
      return <span className="text-ink-500">Too many checks. We&apos;ll confirm when you create your account.</span>;
    case "error":
      return <span className="text-ink-500">We couldn&apos;t check this username right now.</span>;
    default:
      return null;
  }
}

function RegistrationSent({ email, onRestart }: { email: string; onRestart: () => void }) {
  return (
    <div className="animate-rise">
      <span className="inline-flex size-14 items-center justify-center rounded-feature bg-brand-50 text-brand-600">
        <MailCheck aria-hidden className="size-7" />
      </span>
      <h1 tabIndex={-1} className="mt-7 font-display text-[2rem] font-semibold leading-tight text-ink-950 sm:text-4xl">
        Check your inbox
      </h1>
      <p className="mt-3 text-[1.0625rem] leading-relaxed text-ink-600">
        If these details can be used for a new account, we&apos;ve sent a verification link to{" "}
        <span className="font-semibold text-ink-900">{email}</span>. Open it on this device to confirm your email,
        then sign in.
      </p>
      <p className="mt-3 text-[0.9375rem] leading-relaxed text-ink-500">
        For your security we show this message in every case, so it doesn&apos;t reveal whether an account already
        exists. The link expires, so use it soon.
      </p>
      <div className="mt-8 grid gap-3">
        <Button asChild size="lg">
          <Link href="/login">Go to sign in</Link>
        </Button>
        <ResendVerification email={email} />
        <Button variant="ghost" onClick={onRestart}>
          Use a different email
        </Button>
      </div>
    </div>
  );
}
