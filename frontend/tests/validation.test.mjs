import assert from "node:assert/strict";
import { test } from "node:test";
import {
  normalizePhone,
  passwordAccepted,
  passwordRules,
  safeNextPath,
  usernameHandle,
  validEmail,
  validName,
  validPhone,
} from "../lib/validation.ts";

test("email rules mirror the backend's valid_email", () => {
  for (const email of ["ada@example.com", "a.b+c@sub.example.ng", "o'neil@example.com"]) assert.ok(validEmail(email), email);
  for (const email of ["", "ada", "@example.com", ".ada@example.com", "ada.@example.com", "a..b@example.com", "ada@-example.com", "ada@example..com", `${"a".repeat(65)}@example.com`])
    assert.ok(!validEmail(email), email);
});

test("phone numbers must be E.164 after removing spacing", () => {
  assert.equal(normalizePhone("+234 801-234 (5678)"), "+2348012345678");
  assert.ok(validPhone("+234 801 234 5678"));
  for (const phone of ["08012345678", "+0123", "+1", "+1234567890123456", "+234abc"]) assert.ok(!validPhone(phone), phone);
});

test("usernames follow the approved handle format", () => {
  assert.equal(usernameHandle("@ada.okafor_1"), "ada.okafor_1");
  for (const handle of ["ab", "a".repeat(31), ".ada", "ada.", "ada..o", "ada-o", "adá"]) assert.equal(usernameHandle(handle), null, handle);
});

test("password rules match the provisional policy", () => {
  assert.ok(passwordAccepted("A1!xxx"));
  assert.ok(passwordAccepted("Naira€9"));
  for (const password of ["A1!xx", "a1!xxx", "Aa!xxx", "A1xxxx"]) assert.ok(!passwordAccepted(password), password);
  assert.deepEqual(passwordRules("").map(rule => rule.met), [false, false, false, false]);
});

test("names reject control and invisible characters", () => {
  assert.ok(validName("Ada Obi"));
  for (const name of ["", "   ", "Ada​Obi", "Ada\tObi", "A".repeat(101)]) assert.ok(!validName(name), JSON.stringify(name));
});

test("post-sign-in destinations stay inside the account area", () => {
  assert.equal(safeNextPath("/app/security"), "/app/security");
  for (const next of [null, "", "/", "//evil.test", "https://evil.test", "/app/../admin", "/apple", "/app/profile?x=1"]) assert.equal(safeNextPath(next), "/app", String(next));
});
