// Runs the production build against a local HTTP test double of the DEYPAY
// API. The double exists only in this test and answers with the real response
// contracts, so the server-side session guard, redirects and rendering are
// exercised exactly as they run in production. Requires `npm run build`.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { createServer, request as httpRequest } from "node:http";
import { after, before, beforeEach, test } from "node:test";
import { setTimeout as delay } from "node:timers/promises";

const SESSION = "__Host-deypay_session";
const TOKEN = "a".repeat(64);

const profile = {
  id: "0b6f6f9e-4d1c-4f4e-9a4e-6f1f7f5a2b11",
  username: "adaokafor",
  first_name: "Ada",
  last_name: "Okafor",
  email: "ada@example.test",
  email_verified: true,
  phone: "+2348012345678",
  phone_verified: false,
  country_code: "NG",
  account_status: "contact_verified",
  names_editable: true,
  profile_image: null,
  created_at: "2026-11-25T10:00:00Z",
  updated_at: "2026-11-25T10:00:00Z",
};

const legalDocuments = [
  { document_id: "DEP-LEG-001", title: "DEYPAY TERMS OF SERVICE", version: "1.0", sha256: "x", status: "sandbox_draft", url: "/api/v1/legal/documents/DEP-LEG-001/1.0" },
  { document_id: "DEP-LEG-002", title: "DEYPAY PRIVACY NOTICE", version: "1.0", sha256: "y", status: "sandbox_draft", url: "/api/v1/legal/documents/DEP-LEG-002/1.0" },
];

const usdcWallet = {
  id: "6f1c0e2a-1b2c-4d3e-8f90-0123456789ab",
  asset: "USDC",
  asset_name: "USD Coin",
  status: "active",
  balance: { available: "1234567.500000", reserved: "2.000000", total: "1234569.500000" },
  restrictions: ["kyc_required", "financial_activity_not_enabled"],
  capabilities: { deposit: false, send: false },
  created_at: "2026-11-26T10:00:00.000000Z",
  updated_at: "2026-11-26T10:00:00.000000Z",
};
const usdtWallet = {
  ...usdcWallet,
  id: "7f1c0e2a-1b2c-4d3e-8f90-0123456789ab",
  asset: "USDT",
  asset_name: "Tether USD",
  balance: { available: "0.000000", reserved: "0.000000", total: "0.000000" },
};
const VERIFIED_ADDRESS = "0x00000000000000000000000000000000000000a1";
const network = (code, status, address) => ({
  network: code,
  name: code + " fixture",
  chain_family: "evm",
  token_contract: "0x1111111111111111111111111111111111111111",
  token_decimals: 6,
  memo_required: false,
  required_confirmations: 12,
  deposit: { available: status !== "unavailable", address_status: status, address },
  withdrawal: { available: false },
});

const sent = {
  id: "3f7cf7a9-1f2d-4d6b-a830-8cf41a1e9ad4",
  reference: "DP-3F7CF7A91F2D4D6B",
  type: "internal_transfer",
  status: "completed",
  direction: "debit",
  source_asset: "USDT",
  source_amount: "25.500000",
  destination_asset: "USDT",
  destination_amount: "25.500000",
  fee: "0.000000",
  description: "Lunch",
  counterparty: { id: "5c07fb70-71f6-43d8-95b3-a987304679b9", username: "bobmensah", display_name: "Bob Mensah" },
  created_at: "2026-11-27T10:00:00.000000Z",
  completed_at: "2026-11-27T10:00:00.100000Z",
};
const received = {
  ...sent,
  id: "4a7cf7a9-1f2d-4d6b-a830-8cf41a1e9ad5",
  reference: "DP-4A7CF7A91F2D4D6C",
  direction: "credit",
  source_amount: "3.000000",
  destination_amount: "3.000000",
  description: null,
  counterparty: { id: "6c07fb70-71f6-43d8-95b3-a987304679b9", username: "chidiobi", display_name: "Chidi Obi" },
};

let state;
let api;
let web;
let origin;
let output = "";

function reset() {
  state = { ready: true, me: 200, legal: 200, wallets: 200, transactions: 200, send: false, seen: [] };
}

function json(res, status, body) {
  res.writeHead(status, { "content-type": "application/json", "cache-control": "no-store" });
  res.end(JSON.stringify(body));
}

async function listen(server) {
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  return server.address().port;
}

before(async () => {
  reset();
  api = createServer((req, res) => {
    state.seen.push({ url: req.url, cookie: req.headers.cookie ?? null });
    if (req.url.startsWith("/api/v1/wallets")) {
      if (req.headers.cookie !== `${SESSION}=${TOKEN}`) return json(res, 401, { error: { code: "AUTHENTICATION_REQUIRED" } });
      if (state.wallets !== 200) return json(res, state.wallets, { error: { code: "SERVICE_UNAVAILABLE" } });
      if (req.url === "/api/v1/wallets") {
        const capabilities = { deposit: false, send: state.send };
        return json(res, 200, { data: [usdcWallet, usdtWallet].map(wallet => ({ ...wallet, capabilities })) });
      }
      if (req.url === `/api/v1/wallets/${usdcWallet.id}`) {
        return json(res, 200, {
          data: {
            ...usdcWallet,
            networks: [
              network("base-fixture", "active", { address: VERIFIED_ADDRESS, memo: null }),
              network("polygon-fixture", "pending", null),
            ],
          },
        });
      }
      return json(res, 404, { error: { code: "NOT_FOUND", message: "Wallet not found", details: {} } });
    }
    if (req.url.startsWith("/api/v1/transactions")) {
      if (req.headers.cookie !== `${SESSION}=${TOKEN}`) return json(res, 401, { error: { code: "AUTHENTICATION_REQUIRED" } });
      if (state.transactions !== 200) return json(res, state.transactions, { error: { code: "SERVICE_UNAVAILABLE" } });
      if (req.url === "/api/v1/transactions?limit=20") {
        return json(res, 200, { data: [sent, received], meta: { next_cursor: null } });
      }
      if (req.url === `/api/v1/transactions/${sent.id}`) return json(res, 200, { data: sent });
      return json(res, 404, { error: { code: "TRANSACTION_NOT_FOUND", message: "Transaction not found", details: {} } });
    }
    switch (req.url) {
      case "/health/ready":
        return json(res, state.ready ? 200 : 503, { status: state.ready ? "ready" : "unavailable" });
      case "/api/v1/me":
        if (req.headers.cookie !== `${SESSION}=${TOKEN}`) return json(res, 401, { error: { code: "AUTHENTICATION_REQUIRED" } });
        if (state.me === 200) return json(res, 200, { data: profile });
        return json(res, state.me, { error: { code: state.me === 401 ? "AUTHENTICATION_REQUIRED" : "SERVICE_UNAVAILABLE" } });
      case "/api/v1/me/preferences":
        return json(res, 200, {
          data: { phone_discoverable: false, email_notifications: true, push_notifications: false, marketing_consent: false, updated_at: null },
        });
      case "/api/v1/legal/documents":
        if (state.legal !== 200) return json(res, 503, {});
        return json(res, 200, { data: { scope: "sandbox", documents: legalDocuments } });
      case "/api/v1/legal/documents/DEP-LEG-001/1.0":
        res.writeHead(200, { "content-type": "text/markdown; charset=utf-8", "x-deypay-legal-status": "sandbox_draft" });
        return res.end("# DEYPAY TERMS OF SERVICE\n\n**Document ID:** DEP-LEG-001  \nPlain <script>alert(1)</script> text.\n\n- First item\n");
      default:
        return json(res, 404, { error: { code: "NOT_FOUND" } });
    }
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
  for (const stream of [web.stdout, web.stderr]) stream.on("data", chunk => { output = (output + chunk).slice(-4000); });
  for (let attempt = 0; attempt < 160; attempt++) {
    if (web.exitCode !== null) throw new Error(output);
    try { if ((await fetch(`${origin}/status`, { signal: AbortSignal.timeout(1000) })).ok) return; } catch {}
    await delay(250);
  }
  throw new Error("Frontend did not start. " + output);
});

beforeEach(reset);

after(async () => {
  if (web && web.exitCode === null && web.signalCode === null) {
    const closed = once(web, "exit");
    web.kill();
    await closed;
  }
  if (api?.listening) await new Promise(resolve => api.close(resolve));
});

const get = (path, cookie) =>
  fetch(origin + path, { redirect: "manual", headers: cookie ? { cookie } : {} });

const signedIn = `${SESSION}=${TOKEN}`;

test("the status page follows API readiness on successive requests", async () => {
  for (const [ready, label] of [[true, "Available"], [false, "Unavailable"]]) {
    state.ready = ready;
    const response = await get("/status");
    assert.equal(response.status, 200);
    assert.ok((await response.text()).includes(`>${label}</dd>`));
  }
});

test("the landing page renders with a nonce CSP and anti-framing headers", async () => {
  const response = await get("/");
  assert.equal(response.status, 200);
  const csp = response.headers.get("content-security-policy") ?? "";
  assert.match(csp, /script-src 'self' 'nonce-[A-Za-z0-9+/=]+' 'strict-dynamic'/);
  assert.match(csp, /frame-ancestors 'none'/);
  assert.equal(response.headers.get("x-frame-options"), "DENY");
  const html = await response.text();
  assert.ok(html.includes('href="/register"'));
  assert.ok(html.includes("In development"));
  assert.equal(state.seen.length, 0, "the landing page makes no API calls");
});

test("the account area redirects to sign-in without a session cookie", async () => {
  for (const path of ["/app", "/app/profile", "/app/security", "/app/preferences?section=notifications"]) {
    const response = await get(path);
    assert.equal(response.status, 307);
    const location = new URL(response.headers.get("location"), origin);
    assert.equal(location.pathname, "/login");
    assert.equal(location.searchParams.get("next"), new URL(path, origin).pathname);
    assert.equal(response.headers.get("cache-control"), "no-store");
  }
  assert.equal(state.seen.length, 0);
});

test("a rejected session is sent to sign-in without rendering private data", async () => {
  state.me = 401;
  const response = await get("/app", signedIn);
  assert.equal(response.status, 307);
  const location = new URL(response.headers.get("location"), origin);
  assert.equal(location.pathname, "/login");
  assert.equal(location.searchParams.get("reason"), "expired");
  const body = await response.text();
  for (const field of ["id", "email", "phone", "username"]) {
    assert.ok(!body.includes(profile[field]), `rejected HTML response exposed ${field}`);
  }
});

test("profile navigation stops exposing customer data when the API expires a session", async () => {
  const navigate = () => fetch(origin + "/app/profile", {
    redirect: "manual",
    headers: { cookie: signedIn, rsc: "1" },
  });
  const active = await navigate();
  assert.equal(active.status, 200);
  assert.match(active.headers.get("content-type") ?? "", /text\/x-component/);
  assert.ok((await active.text()).includes(profile.email));

  state.me = 401;
  const expired = await navigate();
  // Client navigation carries the redirect in the Flight stream.
  assert.equal(expired.status, 200);
  assert.match(expired.headers.get("content-type") ?? "", /text\/x-component/);
  const body = await expired.text();
  assert.match(body, /NEXT_REDIRECT;replace;\/login\?/);
  for (const field of ["id", "email", "phone", "username"]) {
    assert.ok(!body.includes(profile[field]), `rejected RSC response exposed ${field}`);
  }
});

// Requests as the HTTPS ingress forwards them: the browser's Host and scheme.
function viaIngress(path, cookie) {
  return new Promise((resolve, reject) => {
    const target = new URL(path, origin);
    const headers = { host: "127.0.0.2:8443", "x-forwarded-proto": "https", "x-forwarded-host": "127.0.0.2:8443" };
    if (cookie) headers.cookie = cookie;
    const req = httpRequest({ hostname: target.hostname, port: target.port, path: target.pathname, headers }, res => {
      res.resume();
      res.on("end", () => resolve({ status: res.statusCode, location: res.headers.location }));
    });
    req.on("error", reject);
    req.end();
  });
}

test("sign-in redirects stay on the browser's origin behind the HTTPS ingress", async () => {
  for (const path of ["/app", "/app/security"]) {
    const { status, location } = await viaIngress(path);
    assert.equal(status, 307);
    const target = new URL(location);
    assert.equal(target.origin, "https://127.0.0.2:8443");
    assert.equal(target.pathname, "/login");
    assert.equal(target.searchParams.get("next"), path);
  }
});

test("the session token never appears in rendered pages or RSC payloads", async () => {
  for (const path of ["/", "/login", "/register", "/app", "/app/profile", "/app/preferences", "/app/security"]) {
    for (const rsc of [false, true]) {
      const response = await fetch(origin + path, {
        redirect: "manual",
        headers: rsc ? { cookie: signedIn, rsc: "1" } : { cookie: signedIn },
      });
      const body = await response.text();
      assert.ok(!body.includes(TOKEN), `${path}${rsc ? " (RSC)" : ""} exposed the session token`);
    }
  }
});

test("a valid session renders the customer's own profile and is never cached", async () => {
  const response = await get("/app", `${signedIn}; tracking=1`);
  assert.equal(response.status, 200);
  assert.match(response.headers.get("cache-control") ?? "", /no-store/);
  const html = await response.text();
  assert.ok(html.includes("Ada"));
  assert.ok(html.includes("@adaokafor"));
  assert.ok(html.includes("Not yet available"));
  const lookups = state.seen.filter(entry => entry.url === "/api/v1/me");
  assert.ok(lookups.length >= 1);
  for (const lookup of lookups) assert.equal(lookup.cookie, signedIn, "only the session cookie is forwarded");
});

test("an unavailable account service shows a safe error state", async () => {
  state.me = 503;
  const response = await get("/app/profile", signedIn);
  assert.equal(response.status, 200);
  const html = await response.text();
  assert.ok(html.includes("load your account right now"));
  assert.ok(!html.includes(profile.email));
});

test("preferences render from the API for the signed-in customer", async () => {
  const response = await get("/app/preferences", signedIn);
  assert.equal(response.status, 200);
  assert.ok(state.seen.some(entry => entry.url === "/api/v1/me/preferences" && entry.cookie === signedIn));
});

test("signed-in customers skip sign-in, and only internal destinations are honoured", async () => {
  for (const [next, expected] of [[null, "/app"], ["/app/profile", "/app/profile"], ["https://evil.test/", "/app"], ["//evil.test", "/app"]]) {
    const query = next ? `?next=${encodeURIComponent(next)}` : "";
    const response = await get(`/login${query}`, signedIn);
    assert.equal(response.status, 307);
    assert.equal(new URL(response.headers.get("location"), origin).pathname, expected);
    assert.equal(new URL(response.headers.get("location"), origin).origin, origin);
  }
  const anonymous = await get("/login");
  assert.equal(anonymous.status, 200);
});

test("registration shows the current legal versions or a safe unavailable state", async () => {
  let html = await (await get("/register")).text();
  assert.ok(html.includes("Three short steps"));
  assert.ok(!html.includes("Registration is temporarily unavailable"));
  state.legal = 503;
  html = await (await get("/register")).text();
  assert.ok(html.includes("Registration is temporarily unavailable"));
});

test("token pages send no referrer and are never stored", async () => {
  for (const path of ["/reset-password", "/verify-email"]) {
    const response = await get(path);
    assert.equal(response.status, 200);
    assert.equal(response.headers.get("referrer-policy"), "no-referrer");
    assert.match(response.headers.get("cache-control") ?? "", /no-store/);
  }
});

test("legal documents render Markdown as text, never as markup", async () => {
  const response = await get("/legal/DEP-LEG-001/1.0");
  assert.equal(response.status, 200);
  const html = await response.text();
  assert.ok(html.includes("DEYPAY TERMS OF SERVICE</h1>"));
  assert.ok(html.includes("Sandbox draft"));
  assert.ok(!html.includes("<script>alert(1)</script>"));
  assert.ok(html.includes("&lt;script&gt;alert(1)&lt;/script&gt;"));
  assert.equal((await get("/legal/not-a-document/1.0")).status, 404);
});

test("wallets render the ledger balances the API returns, exactly", async () => {
  const response = await get("/app/wallets", signedIn);
  assert.equal(response.status, 200);
  assert.match(response.headers.get("cache-control") ?? "", /no-store/);
  const html = await response.text();
  for (const shown of ["1,234,569.50", "1,234,567.50", "USDT", "Identity verification is needed"]) {
    assert.ok(html.includes(shown), shown);
  }
  const reads = state.seen.filter(entry => entry.url === "/api/v1/wallets");
  assert.ok(reads.length >= 1);
  for (const read of reads) assert.equal(read.cookie, signedIn, "only the session cookie is forwarded");
});

test("an unavailable wallet service never shows a substitute balance", async () => {
  state.wallets = 503;
  const response = await get("/app/wallets", signedIn);
  assert.equal(response.status, 200);
  const html = await response.text();
  assert.ok(html.includes("load your wallets right now"));
  assert.ok(!html.includes("Total balance"));
  assert.ok(!html.includes("1,234,569.50"));
});

test("wallet details show a deposit address only once it is verified", async () => {
  const response = await get(`/app/wallets/${usdcWallet.id}`, signedIn);
  assert.equal(response.status, 200);
  const html = await response.text();
  assert.ok(html.includes(VERIFIED_ADDRESS));
  assert.ok(html.includes("Copy address"));
  assert.ok(html.includes("Being prepared"), "a pending address is described, never invented");
  assert.ok(html.includes("Not yet available"), "moving money stays unavailable");
});

test("unknown and malformed wallets are not found without leaking anything", async () => {
  const unknown = await get("/app/wallets/42e0fca0-5942-4ef0-974e-ef84594a35ef", signedIn);
  assert.equal(unknown.status, 404);
  const before = state.seen.length;
  const malformed = await get("/app/wallets/not-a-wallet", signedIn);
  assert.equal(malformed.status, 404);
  assert.ok(!state.seen.slice(before).some(entry => entry.url.startsWith("/api/v1/wallets/")), "no API call for a malformed id");
});

test("payments show both directions exactly, with the reference both customers share", async () => {
  const response = await get("/app/payments", signedIn);
  assert.equal(response.status, 200);
  assert.match(response.headers.get("cache-control") ?? "", /no-store/);
  const html = await response.text();
  for (const shown of [
    "Sent to Bob Mensah (@bobmensah)",
    "−25.50",
    "Received from Chidi Obi (@chidiobi)",
    "+3.00",
    sent.reference,
    received.reference,
    "Send to a DEYPAY user",
  ]) {
    assert.ok(html.includes(shown), shown);
  }
  for (const read of state.seen.filter(entry => entry.url.startsWith("/api/v1/transactions"))) {
    assert.equal(read.cookie, signedIn, "only the session cookie is forwarded");
  }
});

test("an unavailable payment service never shows an invented history", async () => {
  state.transactions = 503;
  const html = await (await get("/app/payments", signedIn)).text();
  assert.ok(html.includes("load your payments right now"));
  assert.ok(!html.includes(sent.reference));
});

test("payment details render for a participant; unknown and malformed ids are not found", async () => {
  const response = await get(`/app/payments/${sent.id}`, signedIn);
  assert.equal(response.status, 200);
  const html = await response.text();
  for (const shown of [sent.reference, "Lunch", "Bob Mensah (@bobmensah)", "same reference"]) {
    assert.ok(html.includes(shown), shown);
  }
  assert.equal((await get(`/app/payments/${received.counterparty.id}`, signedIn)).status, 404);
  const before = state.seen.length;
  assert.equal((await get("/app/payments/not-a-payment", signedIn)).status, 404);
  assert.ok(
    !state.seen.slice(before).some(entry => entry.url.startsWith("/api/v1/transactions/")),
    "no API call for a malformed id",
  );
});

test("sending stays closed until a wallet may send, then shows the confirmation flow", async () => {
  const closed = await (await get("/app/payments/send", signedIn)).text();
  assert.ok(closed.includes("Sending isn"), "the closed state is explained");
  assert.ok(!closed.includes("Review transfer"));
  state.send = true;
  const open = await (await get("/app/payments/send", signedIn)).text();
  for (const shown of ["Review transfer", "username", "Available: 1,234,567.50 USDC"]) {
    assert.ok(open.includes(shown), shown);
  }
});
