import { randomBytes } from "node:crypto";
import { writeFile } from "node:fs/promises";

const password = () => randomBytes(32).toString("hex");
const appPassword = password();
const settings = [
  "APP_ENV=local",
  "API_BIND_ADDR=127.0.0.1:8000",
  "API_BASE_URL=http://127.0.0.1:8000",
  "DB_MAX_CONNECTIONS=5",
  "LOG_LEVEL=info",
  `POSTGRES_PASSWORD=${password()}`,
  `DEYPAY_DB_PASSWORD=${appPassword}`,
  `DATABASE_URL=postgresql://deypay_app:${appPassword}@127.0.0.1:5432/deypay`,
];
try {
  await writeFile(new URL("../.env", import.meta.url), settings.join("\n") + "\n", { flag: "wx", mode: 0o600 });
  console.log("Created .env with local development credentials.");
} catch (error) {
  if (error.code !== "EEXIST") throw error;
  console.error("An existing .env was preserved.");
  process.exitCode = 1;
}
