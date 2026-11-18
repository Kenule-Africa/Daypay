import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, rmdir, unlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { parseEnv } from "node:util";

test("local initialization separates credentials and preserves an existing environment", async t => {
  const root = await mkdtemp(join(tmpdir(), "deypay-env-test-"));
  const scripts = join(root, "scripts");
  const init = join(scripts, "init-local.mjs");
  const file = join(root, ".env");
  await mkdir(scripts);
  await copyFile(new URL("./init-local.mjs", import.meta.url), init);
  t.after(async () => {
    await unlink(file).catch(error => { if (error.code !== "ENOENT") throw error; });
    await unlink(init);
    await rmdir(scripts);
    await rmdir(root);
  });

  const first = spawnSync(process.execPath, [init], { encoding: "utf8" });
  assert.equal(first.status, 0);
  const original = await readFile(file, "utf8");
  const settings = parseEnv(original);
  const db = new URL(settings.DATABASE_URL);
  assert.equal(db.hostname, "127.0.0.1");
  assert.equal(db.username, "deypay_app");
  assert.ok(db.password.length >= 32 && db.password === settings.DEYPAY_DB_PASSWORD);
  assert.ok(settings.POSTGRES_PASSWORD.length >= 32 && settings.POSTGRES_PASSWORD !== db.password);
  assert.ok(![db.password, settings.POSTGRES_PASSWORD].some(secret =>
    (first.stdout + first.stderr).includes(secret)));

  const rerun = spawnSync(process.execPath, [init], { encoding: "utf8" });
  assert.equal(rerun.status, 1);
  assert.ok((await readFile(file, "utf8")) === original);
});
