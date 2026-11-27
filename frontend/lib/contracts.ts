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

// Wallet APIs (API §§18–19, backend deypay-api wallets.rs). Amounts are exact
// decimal strings from the DEYPAY ledger; they are never computed here.
export type WalletStatus = "provisioning" | "active" | "restricted" | "suspended" | "closed";

export type WalletRestriction =
  | "wallet_provisioning"
  | "wallet_restricted"
  | "wallet_suspended"
  | "wallet_closed"
  | "account_restricted"
  | "kyc_required"
  | "asset_unavailable"
  | "asset_on_hold"
  | "financial_activity_not_enabled";

export type WalletBalance = { available: string; reserved: string; total: string };

export type Wallet = {
  id: string;
  asset: string;
  asset_name: string;
  status: WalletStatus;
  /** Null only while the wallet's ledger accounts are being opened. */
  balance: WalletBalance | null;
  restrictions: WalletRestriction[];
  capabilities: { deposit: boolean; send: boolean };
  created_at: string;
  updated_at: string;
};

export type AddressStatus = "active" | "pending" | "not_requested" | "unavailable";

export type WalletNetwork = {
  network: string;
  name: string;
  chain_family: "evm" | "solana";
  token_contract: string;
  token_decimals: number;
  memo_required: boolean;
  required_confirmations: number | null;
  deposit: {
    available: boolean;
    address_status: AddressStatus;
    address: { address: string; memo: string | null } | null;
  };
  withdrawal: { available: boolean };
};

export type WalletDetail = Wallet & { networks: WalletNetwork[] };
