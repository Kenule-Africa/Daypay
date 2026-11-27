// Rules and wording for internal transfers. The server validates everything
// again; these checks only stop requests that would certainly be refused.
import type { TransactionStatus } from "./contracts";
import type { Tone } from "./wallets";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function isTransactionId(value: string): boolean {
  return UUID.test(value);
}

/** API §16.7: 3–30 letters, digits, `_` or `.`, optional leading `@`. Returns the handle without `@`. */
export function normalizeHandle(input: string): string | null {
  const handle = input.trim().replace(/^@/, "");
  const valid =
    /^[A-Za-z0-9_.]{3,30}$/.test(handle) && !handle.startsWith(".") && !handle.endsWith(".") && !handle.includes("..");
  return valid ? handle : null;
}

/** A positive plain decimal with at most `decimals` places (USDC and USDT: 6). */
export function validAmount(value: string, decimals = 6): boolean {
  const match = /^(0|[1-9]\d{0,31})(?:\.(\d+))?$/.exec(value.trim());
  if (!match) return false;
  const fraction = match[2] ?? "";
  if (fraction.length > decimals) return false;
  return /[1-9]/.test(match[1] + fraction);
}

export const MAX_NOTE = 140;

/** A fresh Idempotency-Key for one confirmed transfer; retries reuse it. */
export function newIdempotencyKey(): string {
  return crypto.randomUUID();
}

export const transactionStatus: Record<TransactionStatus, { label: string; tone: Tone }> = {
  created: { label: "Created", tone: "neutral" },
  awaiting_authorization: { label: "Awaiting confirmation", tone: "neutral" },
  pending: { label: "Pending", tone: "caution" },
  processing: { label: "Processing", tone: "caution" },
  completed: { label: "Completed", tone: "positive" },
  failed: { label: "Failed", tone: "danger" },
  cancelled: { label: "Cancelled", tone: "neutral" },
  under_review: { label: "Under review", tone: "caution" },
  pending_reconciliation: { label: "Being confirmed", tone: "caution" },
  reversed: { label: "Reversed", tone: "neutral" },
  refunded: { label: "Refunded", tone: "neutral" },
};

/** "@bob · Bob Mensah", or whatever identity details the API permits. */
export function counterpartyName(party: { username: string | null; display_name: string | null }): string {
  if (party.display_name && party.username) return `${party.display_name} (@${party.username})`;
  if (party.username) return `@${party.username}`;
  return party.display_name ?? "A DEYPAY customer";
}

/** Customer wording for transfer refusals. Reasons are never more specific than the API's. */
export function transferFailure(code: string, reason?: string, status = 0): string {
  if (reason === "transfers_not_enabled") return "Sending money on DEYPAY isn't open yet. Your balance is safe.";
  if (reason === "recipient_lookup_not_enabled") return "Finding DEYPAY users isn't available yet.";
  switch (code) {
    case "NOT_FOUND":
    case "RECIPIENT_NOT_FOUND":
      return "No DEYPAY user has that username. Check the spelling and try again.";
    case "RECIPIENT_UNAVAILABLE":
      return "This user can't receive transfers right now.";
    case "SELF_TRANSFER_NOT_PERMITTED":
      return "You can't send money to yourself.";
    case "VALIDATION_ERROR":
      return "Check the details and try again.";
    case "INVALID_AMOUNT":
      return "Enter an amount greater than zero with up to 6 decimal places.";
    case "UNSUPPORTED_ASSET":
      return "Choose USDC or USDT.";
    case "INSUFFICIENT_FUNDS":
      return "You don't have enough available balance for this transfer.";
    case "TRANSACTION_LIMIT_EXCEEDED":
      return "This amount is above what you can send at once.";
    case "KYC_REQUIRED":
      return "Identity verification is needed before you can send money.";
    case "ACCOUNT_RESTRICTED":
      return "Your account can't send money right now. Contact support for help.";
    case "ASSET_UNAVAILABLE":
      return "This asset is temporarily unavailable.";
    case "AUTHORIZATION_INVALID":
      return "This confirmation is no longer valid. Review the transfer and confirm it again.";
    case "IDEMPOTENCY_CONFLICT":
      return "This transfer clashes with an earlier request. Start a new transfer.";
    case "RATE_LIMITED":
      return "Too many lookups. Wait a few minutes, then try again.";
    case "NETWORK_ERROR":
      return "We couldn't reach DEYPAY. Check your connection and try again.";
    default:
      return status >= 500
        ? "Transfers are temporarily unavailable. Please try again shortly."
        : "We couldn't complete that request. Please try again.";
  }
}
