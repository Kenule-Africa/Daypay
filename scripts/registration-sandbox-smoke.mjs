// Invoked by the SQLx registration suite with its migrated, disposable database.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { once } from "node:events";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";

const [binary, database] = process.argv.slice(2);
const url = new URL(process.env.DATABASE_URL);
url.pathname = "/" + database;
url.searchParams.set("options", "-c role=deypay_app");
const dir = await mkdtemp(join(tmpdir(), "deypay-screening-"));
const sha = data => createHash("sha256").update(data).digest("hex");
const blocked = "Compromised-Test1!";
const invalidVerificationToken = "00000000-0000-0000-0000-000000000001." + "ab".repeat(32);
const body = { first_name: "Ada", last_name: "Okafor", country_code: "NG", username: "ada",
  email: "ada@example.test", phone: "+2348012345678", password: "Sandbox-Only1!",
  terms_version: "1.0", privacy_version: "1.0", accept_terms: true, acknowledge_privacy_notice: true };

async function check(environment) {
  const dataset = sha(blocked) + "\n";
  const file = join(dir, "sandbox.sha256");
  await writeFile(file, dataset);
  const proc = spawn(binary, [], { windowsHide: true, stdio: ["ignore", "pipe", "pipe"], env: {
    ...process.env, APP_ENV: environment, API_BIND_ADDR: "127.0.0.1:0", DATABASE_URL: url.href, LOG_LEVEL: "info",
    DEYPAY_REGISTRATION_MODE: "sandbox", DEYPAY_SCREENING_FILE: file, DEYPAY_SCREENING_SHA256: sha(dataset),
    DEYPAY_SCREENING_VALID_UNTIL_UNIX: String(Math.floor(Date.now()/1000)+3600), DEYPAY_ARGON2_MEMORY_KIB: "32",
    DEYPAY_ARGON2_ITERATIONS: "2", DEYPAY_ARGON2_LANES: "1", DEYPAY_HASH_CONCURRENCY: "1",
    DEYPAY_REGISTRATION_ATTEMPTS_PER_MINUTE: "10", EMAIL_VERIFICATION_ENABLED: String(environment==="test")
  }});
  const closed = once(proc, "close");
  const lines = createInterface({input: proc.stdout});
  const output = [];
  proc.stderr.on("data", data => output.push(data.toString()));
  let timer;
  try {
    const address = await new Promise((resolve,reject) => {
      timer = setTimeout(() => reject(Error("API startup timed out")), 60000);
      closed.then(() => reject(Error("API exited during startup")), reject);
      lines.on("line", line => {
        output.push(line);
        try { const {fields}=JSON.parse(line); if(fields?.message==="API listening")resolve(fields.address); }
        catch { reject(Error("Invalid API log")); }
      });
    });
    const origin = "http://" + address;
    const post = data => fetch(origin+"/api/v1/auth/register",{method:"POST",headers:{"content-type":"application/json"},
      body:JSON.stringify(data),signal:AbortSignal.timeout(30000)});
    if (environment === "production") {
      assert.equal((await post(body)).status,503);
    } else {
      const index = await fetch(origin+"/api/v1/legal/documents",{signal:AbortSignal.timeout(30000)});
      assert.equal(index.status,200);
      const {data: {scope,documents}} = await index.json();
      assert.equal(scope,"sandbox");
      assert.equal(documents.length,3);
      for(const doc of documents) {
        const page = await fetch(origin+doc.url,{signal:AbortSignal.timeout(30000)});
        assert.equal(page.status,200);
        assert.equal(page.headers.get("x-deypay-legal-status"),"sandbox_draft");
        assert.equal(sha(Buffer.from(await page.arrayBuffer())),doc.sha256);
      }
      assert.equal((await post({...body,password:blocked})).status,422);
      assert.equal((await post({...body,accept_terms:false})).status,422);
      for(let i=0;i<2;i++){
        const res=await post(body);
        assert.equal(res.status,202);
        assert.equal(res.headers.get("set-cookie"),null);
        assert.deepEqual(await res.json(),{data:{}});
      }
      for (const [path, payload, status] of [
        ["resend-verification", {email:body.email}, 202],
        ["verify-email", {token:invalidVerificationToken}, 422],
      ]) {
        const result = await fetch(origin+"/api/v1/auth/"+path,{method:"POST",headers:{"content-type":"application/json"},
          body:JSON.stringify(payload),signal:AbortSignal.timeout(30000)});
        assert.equal(result.status,status);
        const response = await result.text();
        assert.ok(!response.includes(invalidVerificationToken) && !response.includes(body.email));
      }
    }
  } finally {
    clearTimeout(timer);
    if(proc.exitCode===null && proc.signalCode===null)proc.kill();
    await closed;
    lines.close();
  }
  for (const secret of [blocked,body.password,body.email,invalidVerificationToken]) assert.ok(!output.join("\n").includes(secret),"Sensitive authentication log");
}

try { await check("test"); await check("production"); }
finally { await rm(dir,{recursive:true,force:true}); }
