import assert from "node:assert/strict";
import { test } from "node:test";
import { displayAmount, isWalletId, restrictionText, walletStatus } from "../lib/wallets.ts";

test("ledger amounts are displayed exactly, never rounded", () => {
  for (const [value, shown] of [
    ["0.000000", "0.00"],
    ["12.500000", "12.50"],
    ["5", "5.00"],
    ["100.10", "100.10"],
    ["1234567.123456", "1,234,567.123456"],
    ["0.000001", "0.000001"],
    ["99999999999999999999999999999999.999999", "99,999,999,999,999,999,999,999,999,999,999.999999"],
  ]) {
    assert.equal(displayAmount(value), shown, value);
  }
  for (const value of ["", "-1", "1e3", "01.5", "1.", ".5", "1,000.00", "NaN"]) {
    assert.equal(displayAmount(value), null, value);
  }
});

test("only wallet UUIDs are requested from the API", () => {
  assert.ok(isWalletId("42e0fca0-5942-4ef0-974e-ef84594a35ef"));
  for (const value of ["", "42e0fca0", "../me", "42e0fca0-5942-4ef0-974e-ef84594a35ef/balance"]) {
    assert.ok(!isWalletId(value), value);
  }
});

test("every status and restriction has customer wording", () => {
  for (const status of ["provisioning", "active", "restricted", "suspended", "closed"]) {
    assert.ok(walletStatus[status].label);
  }
  for (const code of [
    "wallet_provisioning", "wallet_restricted", "wallet_suspended", "wallet_closed", "account_restricted",
    "kyc_required", "asset_unavailable", "asset_on_hold", "financial_activity_not_enabled",
  ]) {
    assert.ok(restrictionText[code].endsWith("."), code);
  }
});
