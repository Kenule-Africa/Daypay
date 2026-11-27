// Display rules for wallet data. Amounts arrive as exact decimal strings from
// the DEYPAY ledger; display only groups digits and drops trailing zeros
// beyond two decimals, so nothing is ever rounded.
import type { AddressStatus, WalletRestriction, WalletStatus } from "./contracts";

const WALLET_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function isWalletId(value: string): boolean {
  return WALLET_ID.test(value);
}

/** "1234567.500000" → "1,234,567.50". Returns null for anything that is not an exact amount. */
export function displayAmount(value: string): string | null {
  const match = /^(0|[1-9]\d*)(?:\.(\d+))?$/.exec(value);
  if (!match) return null;
  const whole = match[1].replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  const fraction = (match[2] ?? "").replace(/0+$/, "").padEnd(2, "0");
  return `${whole}.${fraction}`;
}

export type Tone = "positive" | "caution" | "danger" | "neutral";

export const walletStatus: Record<WalletStatus, { label: string; tone: Tone }> = {
  provisioning: { label: "Being set up", tone: "neutral" },
  active: { label: "Active", tone: "positive" },
  restricted: { label: "Restricted", tone: "caution" },
  suspended: { label: "Suspended", tone: "danger" },
  closed: { label: "Closed", tone: "neutral" },
};

/** Customer-facing explanations. They never reveal compliance details. */
export const restrictionText: Record<WalletRestriction, string> = {
  wallet_provisioning: "This wallet is still being set up.",
  wallet_restricted: "Payments out of this wallet are restricted.",
  wallet_suspended: "This wallet is suspended. Contact support for help.",
  wallet_closed: "This wallet is closed. Its history stays available.",
  account_restricted: "Your account has a restriction, so some features are unavailable.",
  kyc_required: "Identity verification is needed before you can move money.",
  asset_unavailable: "This asset is temporarily unavailable.",
  asset_on_hold: "This asset is on hold while DEYPAY completes a review.",
  financial_activity_not_enabled: "Deposits, transfers and withdrawals aren't open yet.",
};

export const addressStatus: Record<AddressStatus, string> = {
  active: "Ready",
  pending: "Being prepared",
  not_requested: "Not requested",
  unavailable: "Not available",
};
