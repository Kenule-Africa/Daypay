# Local development environment (Windows, Git Bash)

How to start, stop and update the DEYPAY workstation environment from a **Git Bash** terminal. Every command below is Git Bash syntax.

This environment is for local testing only. It never contacts email, KYC, wallet or blockchain providers, and no financial operation is active.

## What runs

| Service | Address | Started by |
| --- | --- | --- |
| PostgreSQL 18 (private data directory) | `127.0.0.1:55432`, database `deypay` | environment launcher |
| General API (`deypay-api`) | `http://127.0.0.1:8010` | environment launcher |
| Sign-in service (`deypay-auth`) | `http://127.0.0.1:8011` | environment launcher |
| HTTPS ingress (Caddy) | `https://127.0.0.2:8443` | environment launcher |
| Customer web app (Next.js) | `http://127.0.0.1:3000` | you, in its own terminal |

In the browser, open only **https://127.0.0.2:8443**. The ingress sends `/api/*` to the API, the sign-in routes to the sign-in service, and pages to the web app. Do not browse to `:3000`, `:8010` or `:8011` directly; sign-in cookies work only on the HTTPS address.

Everything private lives outside Git in `%LOCALAPPDATA%\DeyPayAuthDev`: database passwords, runtime configuration, the PostgreSQL data directory, the email outbox and test-account credentials. Never copy anything from there into the repository. Its `RUNBOOK.txt` has workstation-specific notes.

## Shell setup

Paste these into each new Git Bash terminal, or add them to `~/.bashrc`:

```bash
PY="$(cygpath -u "$LOCALAPPDATA")/Programs/Python/Python312/python.exe"
DEYPAY_ENV="$(cygpath -u "$LOCALAPPDATA")/DeyPayAuthDev"
DEYPAY_REPO=/c/Users/USER/Desktop/Projects/DeyPay
```

`DEYPAY_REPO` must be this checkout: the environment's migration script reads migrations from it.

## Start everything

1. **Backend: PostgreSQL, API, sign-in service and HTTPS ingress.**

   ```bash
   "$PY" "$DEYPAY_ENV/admin/services.py" start
   ```

   It prints `Ready at https://127.0.0.2:8443` when all four are up. On a busy machine this can take a few minutes. The API and sign-in service run under their own Windows accounts, so always start them with this command, never by running the `.exe` files yourself.

2. **Frontend**, in a second Git Bash terminal:

   ```bash
   cd "$DEYPAY_REPO/frontend"
   npm run dev
   ```

   Keep this terminal open. The web app needs `frontend/.env.local` (gitignored) containing `API_BASE_URL=http://127.0.0.1:8010`. The first page load after starting compiles the app and can take a minute.

3. Open **https://127.0.0.2:8443**.

### Check status

```bash
"$PY" "$DEYPAY_ENV/admin/services.py" status
```

All four lines should end in `True`. To check the web app through the ingress:

```bash
curl -sk -o /dev/null -w '%{http_code}\n' https://127.0.0.2:8443/login    # expect 200
```

### Stop everything

1. In the frontend terminal, press **Ctrl+C**.
2. Stop the backend:

   ```bash
   "$PY" "$DEYPAY_ENV/admin/services.py" stop
   ```

Stopping keeps all data. The next `start` brings back the same database, accounts and wallets.

## Signed-in testing: development vs production frontend

`npm run dev` is fine for looking at pages. In development mode, however, Next.js copies request cookies, including the session cookie, into debug data the browser can read. For thorough signed-in testing, run the production build instead:

```bash
cd "$DEYPAY_REPO/frontend"
# First stop `npm run dev` with Ctrl+C. Both use the frontend/.next folder,
# and building while the dev server runs breaks the dev server.
npm run build
npm run start -- --port 3000
```

Never run `npm run build` while `npm run dev` is running in the same folder. If you did, stop the dev server and start it again.

## Emails: verification and password-reset links

No email is sent locally. After you register or request a password reset in the browser, deliver the pending emails to the private outbox:

```bash
"$PY" "$DEYPAY_ENV/admin/email-outbox.py"
```

It prints a `cat …` command for each message. Run that command and open the link in the browser. Verification links last 15 minutes and reset links 30 minutes. Each link works once; delete the file afterwards. Details and guards are in [LOCAL-EMAIL-VERIFICATION.md](LOCAL-EMAIL-VERIFICATION.md).

## What you can test

- Registration, email verification, sign-in, sign-out and password reset.
- Dashboard, profile, preferences and security (sessions).
- USDC and USDT wallets at `/app/wallets`. They are created automatically on the first visit, with a zero balance. Deposit addresses cannot be requested, because no network or provider is enabled. That is intentional until verified provider evidence and approvals exist (see [HANDOFF.md](HANDOFF.md)).

## After pulling a new `main`

Do these steps in order only when the merge changed the backend, migrations or database scripts. A frontend-only change needs just a restart of `npm run dev`.

1. **Update the checkout:**

   ```bash
   cd "$DEYPAY_REPO"
   git switch main
   git pull --ff-only
   ```

2. **Back up the database** while it is running:

   ```bash
   export PGPASSWORD="$("$PY" -c 'import json,os;print(json.load(open(os.path.join(os.environ["LOCALAPPDATA"],"DeyPayAuthDev","admin","db.json")))["deypay_dev_dba"])')"
   mkdir -p "$DEYPAY_ENV/run/backups"
   "/c/Program Files/PostgreSQL/18/bin/pg_dump.exe" -w -h 127.0.0.1 -p 55432 -U deypay_dev_dba -Fc \
     -f "$DEYPAY_ENV/run/backups/deypay-$(date +%Y%m%d-%H%M%S).dump" deypay
   ```

   This keeps the password in an environment variable; it is never printed. Keep `PGPASSWORD` set for step 4.

3. **Apply new migrations.** This only applies versions that have not been applied yet, and it refuses to continue if an applied migration file has changed:

   ```bash
   "$PY" "$DEYPAY_ENV/admin/native-migrations.py"
   ```

   It prints `Native PostgreSQL migration transport verified N versions.`

4. **Apply the role and grant scripts** that the merged work lists under **Install** in [HANDOFF.md](HANDOFF.md) or the [README](../../README.md), in the order given there. For example, Phase 5 and 6 need, in this order: `ledger-roles`, `wallet-roles`, `ledger-grants`, `wallet-grants`.

   ```bash
   for script in ledger-roles wallet-roles ledger-grants wallet-grants; do
     "/c/Program Files/PostgreSQL/18/bin/psql.exe" -X -w -q -v ON_ERROR_STOP=1 \
       -h 127.0.0.1 -p 55432 -U deypay_dev_dba -d deypay \
       -f "$DEYPAY_REPO/infrastructure/postgres/$script.psql" || break
   done
   unset PGPASSWORD
   ```

   The grant scripts check themselves and stop with an error if a rule does not hold.

5. **Rebuild the API and sign-in service.** Both are binaries of the `deypay-api` package:

   ```bash
   cd "$DEYPAY_REPO/backend"
   cargo build --locked -p deypay-api --bins
   ```

6. **Install the new binaries:** stop the backend, overwrite the two files in place, then start again.

   ```bash
   "$PY" "$DEYPAY_ENV/admin/services.py" stop
   cp "$DEYPAY_REPO/backend/target/debug/deypay-api.exe"  "$DEYPAY_ENV/api/deypay-api.exe"
   cp "$DEYPAY_REPO/backend/target/debug/deypay-auth.exe" "$DEYPAY_ENV/auth/deypay-auth.exe"
   "$PY" "$DEYPAY_ENV/admin/services.py" start
   ```

7. **Restart the frontend:** Ctrl+C, then `npm run dev`. Reinstall frontend dependencies only if `frontend/package-lock.json` changed.

Do not run `admin/runtime-config.py` to update binaries. It rewrites the runtime configuration and switches email verification and password recovery back off.

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| Sign-in fails with `POST http://127.0.0.1:3000/api/v1/auth/login 404` | The browser is on the frontend's own address. Only the HTTPS ingress routes `/api/*` to the backend. Open **https://127.0.0.2:8443** instead, and make sure the backend `status` shows all four `True`. |
| `start` says `Runtime port already occupied` | A previous run left the API or sign-in service running. Run `status`. If they are not ready but still listening, end them with `taskkill //F //IM deypay-api.exe //IM deypay-auth.exe`, then `start`. |
| `stop` says `Runtime still listening` | Same cause and fix. PostgreSQL is left running; `start` reuses it. |
| A page shows `Cannot find module './vendor-chunks/…'` or a client-manifest error | The frontend's `.next` folder is out of step, usually because `npm run build` ran while `npm run dev` was running. Stop the dev server and start it again. Always start it with `npm run dev`, which handles Windows drive-letter casing. |
| `Account service unavailable` on `/app` pages | The backend is not running, or `frontend/.env.local` is missing `API_BASE_URL`. Check `status`. |
| `Password recovery is temporarily unavailable` | The sign-in service needs `PASSWORD_RECOVERY_ENABLED=true` in its private runtime configuration (already set on this workstation). |
| A request returns 503 once, then works | The machine is overloaded. The local services need roughly 2 GB of free memory. Close idle editors, agent sessions and builds. |
| You need a test account's password | Test-account credentials are stored privately in `$DEYPAY_ENV/run/local-accounts/`. A password changed through the reset flow is not written back there. |
