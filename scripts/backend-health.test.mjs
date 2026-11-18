import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { createServer } from "node:net";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";
import { test } from "node:test";

const binary = fileURLToPath(new URL("../backend/target/debug/deypay-api" + (process.platform === "win32" ? ".exe" : ""), import.meta.url));

async function unusedPort() {
  const socket = createServer();
  socket.listen(0, "127.0.0.1");
  await once(socket, "listening");
  const { port } = socket.address();
  await new Promise(resolve => socket.close(resolve));
  return port;
}

async function checkApi(t, databaseUrl, expectedStatus) {
  const port = await unusedPort();
  const origin = `http://127.0.0.1:${port}`;
  const env = { ...process.env, APP_ENV: "test", DATABASE_URL: databaseUrl, LOG_LEVEL: "info", API_BIND_ADDR: `127.0.0.1:${port}` };
  delete env.POSTGRES_PASSWORD;
  delete env.DEYPAY_DB_PASSWORD;
  const proc = spawn(binary, [], {
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  const stdout = [];
  proc.stdout.on("data", chunk => stdout.push(chunk));
  proc.stderr.resume();
  const exited = once(proc, "exit");
  // Observe spawn errors even if they arrive during startup polling.
  exited.catch(() => {});
  await once(proc, "spawn");
  t.after(async () => {
    if (proc.exitCode === null && proc.signalCode === null) proc.kill();
    await exited;
  });
  let live;
  for (let i = 0; i < 100; i++) {
    if (proc.exitCode !== null) throw new Error("API exited before becoming live");
    try { live = await fetch(origin + "/health/live", { signal: AbortSignal.timeout(500) }); break; } catch {}
    await delay(100);
  }
  assert.equal(live?.status, 200);
  assert.deepEqual(await live.json(), { status: "live" });
  const ready = await fetch(origin + "/health/ready", { signal: AbortSignal.timeout(5000) });
  assert.equal(ready.status, expectedStatus);
  assert.equal(ready.headers.get("cache-control"), "no-store");
  assert.deepEqual(await ready.json(), { status: expectedStatus === 200 ? "ready" : "unavailable" });
  assert.equal((await fetch(origin + "/health/live")).status, 200);
  const lines = Buffer.concat(stdout).toString().trim().split("\n");
  assert.ok(lines.some(line => JSON.parse(line).fields.message === "API listening"));
  assert.ok(!lines.join("\n").includes(databaseUrl));
}

test("readiness verifies an actual PostgreSQL connection", async t => {
  assert.ok(process.env.DATABASE_URL, "Provide the local test database URL");
  await checkApi(t, process.env.DATABASE_URL, 200);
});

test("a database outage changes readiness while the process stays live", async t => {
  const port = await unusedPort();
  await checkApi(t, `postgresql://unused:unused@127.0.0.1:${port}/offline`, 503);
});
