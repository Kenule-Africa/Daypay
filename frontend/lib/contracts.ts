// Response shapes of the implemented DEYPAY APIs (API specification §§13–16,
// backend deypay-api profile.rs, login/controls.rs and legal.rs).

export type AccountStatus =
  | "registered"
  | "contact_unverified"
  | "contact_verified"
  | "kyc_required"
  | "kyc_pending"
  | "kyc_approved"
  | "kyc_rejected"
  | "restricted"
  | "suspended"
  | "closed";

export type ProfileImage = {
  version: string;
  content_type: "image/jpeg";
  width: number;
  height: number;
  updated_at: string;
};

export type Profile = {
  id: string;
  username: string;
  first_name: string;
  last_name: string;
  email: string;
  email_verified: boolean;
  phone: string;
  phone_verified: boolean;
  country_code: string;
  account_status: AccountStatus;
  names_editable: boolean;
  profile_image: ProfileImage | null;
  created_at: string;
  updated_at: string;
};

export type Preferences = {
  phone_discoverable: boolean;
  email_notifications: boolean;
  push_notifications: boolean;
  marketing_consent: boolean;
  updated_at: string | null;
};

export type PreferenceKey = Exclude<keyof Preferences, "updated_at">;

export type Session = {
  session_id: string;
  is_current: boolean;
  created_at: string;
  expires_at: string;
};

export type LegalDocument = {
  document_id: string;
  title: string;
  version: string;
  sha256: string;
  status: "sandbox_draft" | "published";
  url: string;
};

export type LegalIndex = { scope: string; documents: LegalDocument[] };

export const TERMS_ID = "DEP-LEG-001";
export const PRIVACY_ID = "DEP-LEG-002";
export const CONSENT_WORDING_ID = "DEP-LEG-003";

export type ApiErrorBody = {
  error?: {
    code?: string;
    message?: string;
    details?: { reason?: string; fields?: string[] };
  };
};
