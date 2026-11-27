import assert from "node:assert/strict";
import { test } from "node:test";
import {
  counterpartyName,
  isTransactionId,
  newIdempotencyKey,
  normalizeHandle,
  transactionStatus,
  transferFailure,
  validAmount,
} from "../lib/transfers.ts";

test("usernames follow API §16.7 and lose the optional @", () => {
  for (const [input, handle] of [["@Bob_K", "Bob_K"], [" ada.o ", "ada.o"], ["abc", "abc"]]) {
    assert.equal(normalizeHandle(input), handle, input);
  }
  for (const input of ["", "ab", "@@bob", ".bob", "bob.", "a..b", "bob smith", "x".repeat(31), "bob-k"]) {
    assert.equal(normalizeHandle(input), null, input);
  }
});

test("amounts are plain positive decimals at six places at most", () => {
  for (const value of ["1", "0.5", "25.500000", "0.000001", "1000000"]) assert.ok(validAmount(value), value);
  for (const value of ["", "0", "0.000000", "-1", "1e3", "01", "1.", ".5", "1.0000001", "1,000", "NaN"]) {
    assert.ok(!validAmount(value), value);
  }
  assert.ok(validAmount("0.01", 2));
  assert.ok(!validAmount("0.001", 2));
});

test("only transaction UUIDs reach the API, and keys are unique", () => {
  assert.ok(isTransactionId("3f7cf7a9-1f2d-4d6b-a830-8cf41a1e9ad4"));
  for (const value of ["", "3f7cf7a9", "../me", "3f7cf7a9-1f2d-4d6b-a830-8cf41a1e9ad4/receipt"]) {
    assert.ok(!isTransactionId(value), value);
  }
  const key = newIdempotencyKey();
  assert.match(key, /^[A-Za-z0-9_.:-]{8,128}$/, "the API's Idempotency-Key format");
  assert.notEqual(key, newIdempotencyKey());
});

test("every status and refusal has customer wording without inventing reasons", () => {
  for (const status of [
    "created", "awaiting_authorization", "pending", "processing", "completed", "failed", "cancelled",
    "under_review", "pending_reconciliation", "reversed", "refunded",
  ]) {
    assert.ok(transactionStatus[status].label, status);
  }
  for (const code of [
    "NOT_FOUND", "RECIPIENT_NOT_FOUND", "RECIPIENT_UNAVAILABLE", "SELF_TRANSFER_NOT_PERMITTED", "INVALID_AMOUNT",
    "INSUFFICIENT_FUNDS", "TRANSACTION_LIMIT_EXCEEDED", "KYC_REQUIRED", "ACCOUNT_RESTRICTED", "ASSET_UNAVAILABLE",
    "AUTHORIZATION_INVALID", "IDEMPOTENCY_CONFLICT", "RATE_LIMITED", "NETWORK_ERROR",
  ]) {
    assert.ok(transferFailure(code).endsWith("."), code);
  }
  assert.match(transferFailure("SERVICE_UNAVAILABLE", "transfers_not_enabled", 503), /isn't open yet/);
  assert.match(transferFailure("SERVICE_UNAVAILABLE", undefined, 503), /temporarily unavailable/);
  assert.equal(counterpartyName({ username: "bob", display_name: "Bob Mensah" }), "Bob Mensah (@bob)");
  assert.equal(counterpartyName({ username: null, display_name: null }), "A DEYPAY customer");
});
