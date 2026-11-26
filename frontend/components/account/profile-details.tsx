"use client";

import { AtSign, BadgeCheck, Clock, Globe, Lock, Mail, Phone } from "lucide-react";
import * as React from "react";
import { useAccount } from "@/components/account/account-context";
import { PageHeader, Section } from "@/components/account/page-header";
import { ProfilePhoto } from "@/components/account/profile-photo";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/field";
import { Notice } from "@/components/ui/notice";
import { accountStatus, formatDay } from "@/lib/account";
import { describeFailure, request } from "@/lib/api";
import type { Profile } from "@/lib/contracts";
import { countryName } from "@/lib/countries";
import { validName } from "@/lib/validation";

export function ProfileDetails() {
  const { profile } = useAccount();
  return (
    <>
      <PageHeader eyebrow="Profile" title="Your details" description="The information DEYPAY holds for your account." />
      <div className="grid gap-5 lg:gap-6">
        <Section title="Profile photo" description="Shown to you in DEYPAY. Only you can see or change it.">
          <ProfilePhoto />
        </Section>
        <NameForm />
        <Section title="Contact" description="Changing your email or phone number isn't available yet.">
          <dl className="grid gap-1">
            <Detail icon={Mail} term="Email" value={profile.email}>
              {profile.email_verified ? (
                <Badge tone="live">
                  <BadgeCheck aria-hidden /> Verified
                </Badge>
              ) : (
                <Badge tone="caution">Not verified</Badge>
              )}
            </Detail>
            <Detail icon={Phone} term="Mobile number" value={profile.phone}>
              {profile.phone_verified ? (
                <Badge tone="live">
                  <BadgeCheck aria-hidden /> Verified
                </Badge>
              ) : (
                <Badge tone="soon">
                  <Clock aria-hidden /> Verification coming later
                </Badge>
              )}
            </Detail>
          </dl>
        </Section>
        <Section title="Account">
          <dl className="grid gap-1">
            <Detail icon={AtSign} term="Username" value={`@${profile.username}`}>
              <span className="text-xs text-ink-400">Can&apos;t be changed yet</span>
            </Detail>
            <Detail icon={Globe} term="Country of residence" value={countryName(profile.country_code)} />
            <Detail icon={BadgeCheck} term="Account status" value={accountStatus[profile.account_status].label} />
            <Detail icon={Clock} term="Member since" value={formatDay(profile.created_at)} />
          </dl>
        </Section>
      </div>
    </>
  );
}

function Detail({
  icon: Icon,
  term,
  value,
  children,
}: {
  icon: typeof Mail;
  term: string;
  value: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2 rounded-panel py-3 sm:flex-nowrap">
      <span className="inline-flex size-10 shrink-0 items-center justify-center rounded-full bg-paper text-ink-500">
        <Icon aria-hidden className="size-[1.1rem]" />
      </span>
      <div className="min-w-0 flex-1">
        <dt className="text-sm text-ink-500">{term}</dt>
        <dd className="truncate font-medium text-ink-900">{value}</dd>
      </div>
      {children && <div className="pl-14 sm:pl-0">{children}</div>}
    </div>
  );
}

/** PATCH /api/v1/me with first_name and last_name while the names remain editable. */
function NameForm() {
  const { profile, setProfile } = useAccount();
  const [first, setFirst] = React.useState(profile.first_name);
  const [last, setLast] = React.useState(profile.last_name);
  const [errors, setErrors] = React.useState<{ first?: string; last?: string }>({});
  const [message, setMessage] = React.useState<{ tone: "success" | "error"; text: string } | null>(null);
  const [pending, setPending] = React.useState(false);
  const changed = first.trim() !== profile.first_name || last.trim() !== profile.last_name;

  if (!profile.names_editable) {
    return (
      <Section
        title="Legal name"
        description="Your name is locked because it's used for identity checks."
        action={
          <Badge tone="soon">
            <Lock aria-hidden /> Locked
          </Badge>
        }
      >
        <p className="font-display text-xl font-semibold text-ink-900">
          {profile.first_name} {profile.last_name}
        </p>
      </Section>
    );
  }

  async function save(event: React.FormEvent) {
    event.preventDefault();
    setMessage(null);
    const found = {
      first: validName(first) ? undefined : "Enter your first name (up to 100 characters).",
      last: validName(last) ? undefined : "Enter your last name (up to 100 characters).",
    };
    setErrors(found);
    if (found.first || found.last) return;
    const body: Record<string, string> = {};
    if (first.trim() !== profile.first_name) body.first_name = first.trim();
    if (last.trim() !== profile.last_name) body.last_name = last.trim();
    if (Object.keys(body).length === 0) return;
    setPending(true);
    const result = await request<Profile>("/api/v1/me", { method: "PATCH", json: body });
    setPending(false);
    if (result.ok && result.data) {
      setProfile(result.data);
      setFirst(result.data.first_name);
      setLast(result.data.last_name);
      setMessage({ tone: "success", text: "Your name has been updated." });
      return;
    }
    if (result.ok) return;
    if (result.reason === "names_locked") {
      setProfile({ ...profile, names_editable: false });
      return;
    }
    if (result.status === 422 && result.fields?.length) {
      setErrors({
        first: result.fields.includes("first_name") ? "This first name can't be used." : undefined,
        last: result.fields.includes("last_name") ? "This last name can't be used." : undefined,
      });
      return;
    }
    setMessage({ tone: "error", text: describeFailure(result, "Profile updates") });
  }

  return (
    <Section title="Legal name" description="Use your name exactly as it appears on your government ID.">
      <form noValidate onSubmit={save} className="grid gap-5">
        <div className="grid gap-5 sm:grid-cols-2">
          <Field label="First name" error={errors.first}>
            {({ id, describedBy, invalid }) => (
              <Input
                id={id}
                autoComplete="given-name"
                value={first}
                onChange={event => setFirst(event.target.value)}
                aria-describedby={describedBy}
                invalid={invalid}
                disabled={pending}
              />
            )}
          </Field>
          <Field label="Last name" error={errors.last}>
            {({ id, describedBy, invalid }) => (
              <Input
                id={id}
                autoComplete="family-name"
                value={last}
                onChange={event => setLast(event.target.value)}
                aria-describedby={describedBy}
                invalid={invalid}
                disabled={pending}
              />
            )}
          </Field>
        </div>
        {message && <Notice tone={message.tone}>{message.text}</Notice>}
        <div className="flex flex-wrap items-center gap-3">
          <Button type="submit" loading={pending} disabled={!changed}>
            Save changes
          </Button>
          {changed && !pending && (
            <Button
              type="button"
              variant="ghost"
              onClick={() => {
                setFirst(profile.first_name);
                setLast(profile.last_name);
                setErrors({});
              }}
            >
              Cancel
            </Button>
          )}
        </div>
      </form>
    </Section>
  );
}
