import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const service = process.argv[2];
if (!["api", "worker"].includes(service)) throw new Error("Choose api or worker");
const env = { ...process.env };
delete env.POSTGRES_PASSWORD;
delete env.DEYPAY_DB_PASSWORD;
if (service === "api") {
  delete env.RESEND_API_KEY;
  delete env.EMAIL_VERIFY_TOKEN_KEY;
}

const proc = spawn("cargo", ["run", "--locked", "--bin", `deypay-${service}`], {
  cwd: fileURLToPath(new URL("../backend/", import.meta.url)),
  env,
  stdio: "inherit",
});
proc.on("error", error => {
  console.error("Cargo could not start:", error.code);
  process.exitCode = 1;
});
proc.on("exit", code => { process.exitCode = code ?? 1; });
