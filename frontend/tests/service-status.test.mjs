import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { createServer } from "node:http";
import { after, before, test } from "node:test";
import { setTimeout as delay } from "node:timers/promises";

let status = "ready";
let api;
let web;
let origin;
let output = "";

async function listen(server) {
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  return server.address().port;
}

before(async () => {
  api = createServer((req, res) => {
    assert.equal(req.url, "/health/ready");
    res.writeHead(status === "ready" ? 200 : 503, { "content-type": "application/json" });
    res.end(JSON.stringify({ status }));
  });
  const apiPort = await listen(api);
  const probe = createServer();
  const webPort = await listen(probe);
  await new Promise(resolve => probe.close(resolve));
  origin = `http://127.0.0.1:${webPort}`;
  web = spawn(process.execPath, ["node_modules/next/dist/bin/next", "start", "-H", "127.0.0.1", "-p", String(webPort)], {
    env: { ...process.env, API_BASE_URL: `http://127.0.0.1:${apiPort}`, NEXT_TELEMETRY_DISABLED: "1" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  for (const stream of [web.stdout, web.stderr]) stream.on("data", chunk => { output = (output + chunk).slice(-2000); });
  for (let attempt = 0; attempt < 120; attempt++) {
    if (web.exitCode !== null) throw new Error(output);
    try { if ((await fetch(origin, { signal: AbortSignal.timeout(1000) })).ok) return; } catch {}
    await delay(250);
  }
  throw new Error("Frontend did not start. " + output);
});

after(async () => {
  if (web && web.exitCode === null && web.signalCode === null) {
    const closed = once(web, "exit");
    web.kill();
    await closed;
  }
  if (api?.listening) await new Promise(resolve => api.close(resolve));
});

test("the rendered status follows API readiness on successive requests", async () => {
  for (const [value, label] of [["ready", "Available"], ["unavailable", "Unavailable"]]) {
    status = value;
    const response = await fetch(origin);
    assert.equal(response.status, 200);
    assert.ok((await response.text()).includes(`>${label}</dd>`));
  }
});
