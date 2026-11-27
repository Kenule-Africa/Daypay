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

// Internal transfer APIs (API §§16.2, 20, 28; backend deypay-api transfers.rs).
// Amounts are exact decimal strings at the asset's accounting precision.
export type Recipient = {
  recipient_id: string;
  username: string;
  display_name: string | null;
  can_receive: boolean;
};

export type TransferChallenge = {
  challenge_id: string;
  operation_type: "internal_transfer";
  expires_at: string;
  verification_method: "confirmation";
  terms: {
    recipient: { id: string; username: string | null; display_name: string | null };
    asset: string;
    amount: string;
    fee: string;
    total: string | null;
    description: string | null;
  };
};

export type TransferProof = { authorization_token: string; expires_at: string };

export type TransactionStatus =
  | "created"
  | "awaiting_authorization"
  | "pending"
  | "processing"
  | "completed"
  | "failed"
  | "cancelled"
  | "under_review"
  | "pending_reconciliation"
  | "reversed"
  | "refunded";

export type Counterparty = { id: string; username: string | null; display_name: string | null };

export type TransferResult = {
  transaction_id: string;
  reference: string;
  type: "internal_transfer";
  status: TransactionStatus;
  asset: string;
  amount: string;
  fee: string;
  description: string | null;
  recipient: Counterparty;
  created_at: string;
  completed_at: string | null;
};

export type Transaction = {
  id: string;
  reference: string;
  type: "internal_transfer";
  status: TransactionStatus;
  direction: "debit" | "credit";
  source_asset: string;
  source_amount: string;
  destination_asset: string;
  destination_amount: string;
  fee: string;
  description: string | null;
  counterparty: Counterparty;
  created_at: string;
  completed_at: string | null;
};
