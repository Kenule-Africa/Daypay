import assert from "node:assert/strict";
import { test } from "node:test";
import { canonicalProjectDir } from "../scripts/project-path.mjs";

test("Windows project paths start from one canonical drive-letter spelling", () => {
  assert.equal(canonicalProjectDir("c:\\Users\\dev\\frontend", "win32"), "C:\\Users\\dev\\frontend");
  assert.equal(canonicalProjectDir("C:\\Users\\dev\\frontend", "win32"), "C:\\Users\\dev\\frontend");
  assert.equal(canonicalProjectDir("\\\\server\\share\\frontend", "win32"), "\\\\server\\share\\frontend");
});

test("case-sensitive platforms keep the path unchanged", () => {
  assert.equal(canonicalProjectDir("/home/dev/c:/frontend", "linux"), "/home/dev/c:/frontend");
  assert.equal(canonicalProjectDir("c:/odd/but/valid", "darwin"), "c:/odd/but/valid");
});
