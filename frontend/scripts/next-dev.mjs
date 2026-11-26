// `npm run dev`: starts the Next.js development server from the canonical project
// path (see project-path.mjs). Extra arguments are passed through, for example
// `npm run dev -- --port 3001`.
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { canonicalProjectDir } from "./project-path.mjs";

const root = canonicalProjectDir(join(dirname(fileURLToPath(import.meta.url)), ".."));
const next = createRequire(join(root, "package.json")).resolve("next/dist/bin/next");
const child = spawn(process.execPath, [next, "dev", "--hostname", "127.0.0.1", ...process.argv.slice(2)], {
  cwd: root,
  stdio: "inherit",
});
for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => child.kill(signal));
child.on("exit", (code, signal) => process.exit(code ?? (signal ? 1 : 0)));
