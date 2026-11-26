import type { AccountStatus, Profile } from "./contracts";

export type StatusTone = "positive" | "caution" | "danger" | "neutral";

/** Customer-facing description of the FRS §8.4 lifecycle status. It is not a KYC decision or financial authorization. */
export const accountStatus: Record<AccountStatus, { label: string; tone: StatusTone; detail: string }> = {
  registered: {
    label: "Registered",
    tone: "neutral",
    detail: "Your account has been created. Confirm your email to start using it.",
  },
  contact_unverified: {
    label: "Email not confirmed",
    tone: "caution",
    detail: "Confirm your email address to keep using your account.",
  },
  contact_verified: {
    label: "Active",
    tone: "positive",
    detail:
      "Your email is verified and your account is ready for profile and security features. Financial features will ask for identity verification when they open.",
  },
  kyc_required: {
    label: "Identity verification needed",
    tone: "caution",
    detail: "Financial features will need identity verification. Verification isn't available in the app yet.",
  },
  kyc_pending: {
    label: "Identity verification in review",
    tone: "caution",
    detail: "Your identity verification is being reviewed.",
  },
  kyc_approved: {
    label: "Identity verified",
    tone: "positive",
    detail: "Your identity has been verified. Financial features will open as they become available.",
  },
  kyc_rejected: {
    label: "Identity verification unsuccessful",
    tone: "danger",
    detail: "Your identity verification wasn't approved.",
  },
  restricted: {
    label: "Restricted",
    tone: "danger",
    detail: "Some account features are currently limited.",
  },
  suspended: { label: "Suspended", tone: "danger", detail: "This account is suspended." },
  closed: { label: "Closed", tone: "danger", detail: "This account is closed." },
};

export function fullName(profile: Pick<Profile, "first_name" | "last_name">): string {
  return `${profile.first_name} ${profile.last_name}`.trim();
}

export function initials(profile: Pick<Profile, "first_name" | "last_name">): string {
  const letters = [profile.first_name, profile.last_name].map(name => [...name.trim()][0] ?? "").join("");
  return letters.toUpperCase() || "D";
}

/** A day-precision date in UTC, identical on server and client renders. */
export function formatDay(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" }).format(date);
}

/** Local date and time, for content that renders only in the browser. */
export function formatMoment(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(date);
}

export function profileImageUrl(profile: Profile): string | null {
  return profile.profile_image ? `/api/v1/me/profile-image?v=${encodeURIComponent(profile.profile_image.version)}` : null;
}
