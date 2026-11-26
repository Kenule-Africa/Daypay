"use client";

import { Bell, Mail, Megaphone, Phone, RotateCw } from "lucide-react";
import * as React from "react";
import { useAccount } from "@/components/account/account-context";
import { PageHeader, Section } from "@/components/account/page-header";
import { Button } from "@/components/ui/button";
import { Notice } from "@/components/ui/notice";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { describeFailure, request } from "@/lib/api";
import type { PreferenceKey, Preferences } from "@/lib/contracts";

type Option = { key: PreferenceKey; icon: typeof Mail; title: string; description: string };

const notificationOptions: Option[] = [
  {
    key: "email_notifications",
    icon: Mail,
    title: "Email notifications",
    description: "Optional account updates by email. Essential security and service emails are always sent.",
  },
  {
    key: "push_notifications",
    icon: Bell,
    title: "Push notifications",
    description: "Applies once DEYPAY push notifications are available on your devices.",
  },
  {
    key: "marketing_consent",
    icon: Megaphone,
    title: "Product news and offers",
    description:
      "Optional DEYPAY product news, feature announcements and promotional messages. You can withdraw at any time.",
  },
];

/** GET and PATCH /api/v1/me/preferences. Each switch saves on its own and shows the stored value. */
export function PreferencesForm({ initial }: { initial: Preferences | null }) {
  const { profile } = useAccount();
  const [preferences, setPreferences] = React.useState(initial);
  const [saving, setSaving] = React.useState<PreferenceKey | null>(null);
  const [message, setMessage] = React.useState<{ tone: "success" | "error"; text: string } | null>(null);
  const [loading, setLoading] = React.useState(false);

  async function reload() {
    setLoading(true);
    const result = await request<Preferences>("/api/v1/me/preferences");
    setLoading(false);
    if (result.ok && result.data) setPreferences(result.data);
    else if (!result.ok) setMessage({ tone: "error", text: describeFailure(result, "Preferences") });
  }

  async function change(key: PreferenceKey, value: boolean) {
    if (!preferences) return;
    setSaving(key);
    setMessage(null);
    const result = await request<Preferences>("/api/v1/me/preferences", { method: "PATCH", json: { [key]: value } });
    setSaving(null);
    if (result.ok && result.data) {
      setPreferences(result.data);
      setMessage({ tone: "success", text: "Preference saved." });
    } else if (!result.ok && result.reason === "phone_unverified") {
      setMessage({ tone: "error", text: "Phone discovery needs a verified phone number." });
    } else if (!result.ok) {
      setMessage({ tone: "error", text: `Your change wasn't saved. ${describeFailure(result, "Preferences")}` });
    }
  }

  return (
    <>
      <PageHeader
        eyebrow="Preferences"
        title="How DEYPAY reaches you"
        description="Choose optional messages and how other customers can find you."
      />
      {!preferences ? (
        <Section title="Preferences unavailable">
          <p className="text-ink-600">We couldn&apos;t load your preferences right now.</p>
          <Button variant="soft" className="mt-5" onClick={reload} loading={loading}>
            <RotateCw aria-hidden /> Try again
          </Button>
          {message && (
            <Notice tone={message.tone} className="mt-4">
              {message.text}
            </Notice>
          )}
        </Section>
      ) : (
        <div className="grid gap-5 lg:gap-6">
          <Section title="Notifications">
            <ul className="grid gap-1">
              {notificationOptions.map(option => (
                <PreferenceRow
                  key={option.key}
                  option={option}
                  checked={preferences[option.key]}
                  saving={saving === option.key}
                  disabled={saving !== null}
                  onChange={value => change(option.key, value)}
                />
              ))}
            </ul>
          </Section>
          <Section title="Discovery">
            <ul>
              <PreferenceRow
                option={{
                  key: "phone_discoverable",
                  icon: Phone,
                  title: "Let customers find me by phone number",
                  description: profile.phone_verified
                    ? "Other DEYPAY customers who know your number can find your account."
                    : "Needs a verified phone number. Phone verification isn't available yet.",
                }}
                checked={preferences.phone_discoverable}
                saving={saving === "phone_discoverable"}
                disabled={saving !== null || (!profile.phone_verified && !preferences.phone_discoverable)}
                onChange={value => change("phone_discoverable", value)}
              />
            </ul>
          </Section>
          <div aria-live="polite">
            {message && <Notice tone={message.tone}>{message.text}</Notice>}
          </div>
        </div>
      )}
    </>
  );
}

function PreferenceRow({
  option,
  checked,
  saving,
  disabled,
  onChange,
}: {
  option: Option;
  checked: boolean;
  saving: boolean;
  disabled: boolean;
  onChange: (value: boolean) => void;
}) {
  const id = React.useId();
  const Icon = option.icon;
  return (
    <li className="flex items-start gap-4 py-3">
      <span className="inline-flex size-10 shrink-0 items-center justify-center rounded-full bg-paper text-ink-500">
        <Icon aria-hidden className="size-[1.1rem]" />
      </span>
      <div className="min-w-0 flex-1">
        <label htmlFor={id} className="font-medium text-ink-900">
          {option.title}
        </label>
        <p id={`${id}-description`} className="mt-0.5 text-sm leading-relaxed text-ink-500">
          {option.description}
        </p>
      </div>
      <div className="flex items-center gap-3 pt-1.5">
        {saving && <Spinner label="Saving" className="text-ink-400" />}
        <Switch
          id={id}
          checked={checked}
          disabled={disabled}
          aria-describedby={`${id}-description`}
          onCheckedChange={onChange}
        />
      </div>
    </li>
  );
}
