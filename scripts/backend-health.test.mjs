import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { createServer } from "node:net";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";
import { test } from "node:test";

function serviceBinary(service) {
  const suffix = process.platform === "win32" ? ".exe" : "";
  return fileURLToPath(new URL(`../backend/target/debug/deypay-${service}${suffix}`, import.meta.url));
}

function runtimeEnvironment(databaseUrl) {
  const env = { ...process.env, APP_ENV: "test", DATABASE_URL: databaseUrl, LOG_LEVEL: "info" };
  delete env.POSTGRES_PASSWORD;
  delete env.DEYPAY_DB_PASSWORD;
  return env;
}

function checkLogCredentials(output, url) {
  const password = decodeURIComponent(new URL(url).password);
  assert.ok(!output.includes(url), "Service output disclosed DATABASE_URL");
  if (password) assert.ok(!output.includes(password), "Service output disclosed the database password");
}

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
  const proc = spawn(serviceBinary("api"), [], {
    env: { ...runtimeEnvironment(databaseUrl), API_BIND_ADDR: `127.0.0.1:${port}` },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const stdout = [];
  const stderr = [];
  proc.stdout.on("data", chunk => stdout.push(chunk));
  proc.stderr.on("data", chunk => stderr.push(chunk));
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
  const password = "Synthetic-Registration1!";
  const registration = await fetch(origin + "/api/v1/auth/register", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ email: "smoke@example.test", phone: "+2348012345678", username: "smoke", password }),
  });
  assert.equal(registration.status, 503);
  assert.equal(registration.headers.get("cache-control"), "no-store");
  assert.equal(registration.headers.get("set-cookie"), null);
  assert.equal((await registration.json()).error.code, "SERVICE_UNAVAILABLE");
  const output = Buffer.concat(stdout).toString().trim();
  const logs = output + Buffer.concat(stderr).toString();
  assert.ok(!logs.includes(password) && !logs.includes("smoke@example.test"));
  checkLogCredentials(output, databaseUrl);
  checkLogCredentials(Buffer.concat(stderr).toString(), databaseUrl);
  const lines = output.split("\n");
  assert.ok(lines.some(line => JSON.parse(line).fields.message === "API listening"));
}

test("readiness verifies an actual PostgreSQL connection", async t => {
  assert.ok(process.env.DATABASE_URL, "Provide the local test database URL");
  await checkApi(t, process.env.DATABASE_URL, 200);
});

test("a database outage changes readiness while the process stays live", async t => {
  const port = await unusedPort();
  await checkApi(t, `postgresql://unused:unused@127.0.0.1:${port}/offline`, 503);
});

const workerStarted = "Worker connected; no job handlers registered";

async function checkWorker(databaseUrl, stopOnStartup) {
  const worker = spawn(serviceBinary("worker"), [], {
    env: runtimeEnvironment(databaseUrl),
    stdio: ["ignore", "pipe", "pipe"],
    timeout: 15000,
    killSignal: "SIGKILL",
  });
  const closed = once(worker, "close");
  closed.catch(() => {}); // A spawn error can arrive before stdout ends.
  const stderr = [];
  worker.stderr.on("data", chunk => stderr.push(chunk));
  const messages = [];
  try {
    for await (const line of createInterface({ input: worker.stdout })) {
      checkLogCredentials(line, databaseUrl);
      const message = JSON.parse(line).fields.message;
      messages.push(message);
      if (stopOnStartup && message === workerStarted) worker.kill("SIGTERM");
    }
    const [code, signal] = await closed;
    assert.equal(signal, null, "Worker must exit by itself within the timeout");
    checkLogCredentials(Buffer.concat(stderr).toString(), databaseUrl);
    return { code, messages };
  } finally {
    if (worker.exitCode === null && worker.signalCode === null) worker.kill("SIGKILL");
    await closed;
  }
}

test("worker connects to PostgreSQL and shuts down gracefully on SIGTERM", {
  skip: process.platform === "win32" && "Run the POSIX signal check on the Linux CI runner",
}, async () => {
  assert.ok(process.env.DATABASE_URL, "Provide the local test database URL");
  const { code, messages } = await checkWorker(process.env.DATABASE_URL, true);
  assert.equal(code, 0);
  assert.ok(messages.includes(workerStarted));
  assert.ok(messages.indexOf("Worker stopped") > messages.indexOf(workerStarted));
});

test("worker rejects startup when the database is unreachable", async () => {
  const port = await unusedPort();
  const { code, messages } = await checkWorker(`postgresql://unused:unused@127.0.0.1:${port}/offline`, false);
  assert.equal(code, 1);
  assert.ok(messages.includes("Worker database check failed"));
  assert.ok(!messages.includes(workerStarted));
});
