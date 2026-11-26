# DEYPAY

The Phase 1 foundation contains a Rust API and worker, a Next.js PWA frontend foundation and a local PostgreSQL service. Delivery follows [the implementation roadmap](docs/08-IMPLEMENTATION-PLAN.md). Current verification evidence and outstanding setup requirements are in [the engineering handoff](docs/engineering/HANDOFF.md).

## Local development

Use stable Rust with rustfmt and Clippy, Node.js 22 LTS and PostgreSQL 16 or newer. On Windows, use an MSVC Developer PowerShell with its Windows SDK. Docker Compose v2 provides the containerized local database option.

1. From the repository root, run `node scripts/init-local.mjs` to create an ignored `.env` with two random local passwords. It preserves an existing file. The API and worker use only the runtime database login.
2. Start PostgreSQL with `docker compose up -d --wait postgres`. Data persists in a named development volume. The bootstrap creates a login with database connection and schema usage privileges on first initialization. Keep the generated credentials with that volume across restarts.
3. Verify the application login with `docker compose exec -T postgres sh -ec 'PGPASSWORD="$DEYPAY_DB_PASSWORD" psql -X -h 127.0.0.1 -U deypay_app -d deypay -f /opt/deypay/verify.psql'` from Bash or PowerShell. This checks the server version, database identity and privileges. For a native local instance, have its administrator create a fresh `deypay` database, then run `psql -X -d deypay -f infrastructure/postgres/bootstrap.psql` with the generated `DEYPAY_DB_PASSWORD` exported. Run `verify.psql` over TCP as `deypay_app`.
4. From separate terminals, run `node --env-file=.env scripts/run-local.mjs api` and `node --env-file=.env scripts/run-local.mjs worker`. The worker verifies its database connection at startup. Job handling belongs to later phases.
5. In `frontend/`, run `npm ci` and `npm run dev`. Open `http://127.0.0.1:3000`. The server reads the general API at `http://127.0.0.1:8000`; set server-side `API_BASE_URL` when running the processes on different hosts. See [Customer web app](#customer-web-app) for the browser routing contract.

## Service configuration

| Setting | Behavior |
|---|---|
| APP_ENV | Required. One of local, test, staging or production. Staging and production enforce certificate and hostname verification for PostgreSQL TCP connections. |
| DATABASE_URL | Required PostgreSQL URL for the runtime login. Inject as a secret. |
| API_BIND_ADDR | Defaults to 127.0.0.1:8000. Override deliberately for a hosted listener behind TLS ingress. |
| DB_MAX_CONNECTIONS | Per-process limit, 1–100, default 5. Budget for all API and worker replicas. |
| LOG_LEVEL | error, warn, info or debug. Defaults to info. Services emit JSON events to standard output. |

`GET /health/live` checks the running HTTP process. `GET /health/ready` performs a bounded database query and returns HTTP 200 or 503. Probe responses carry only a status and disable caching.

## Phase 2 database foundation

Use PostgreSQL 16+ and SQLx CLI 0.8.6. From `backend/`, set `DATABASE_URL` in the migration process to the schema owner's database connection. Use verified TLS for hosted connections. Run `sqlx migrate run --source migrations`, then `sqlx migrate info --source migrations`. Reapplying the same release verifies checksums and skips applied versions. Run `psql -X -v ON_ERROR_STOP=1 -1 -f ../infrastructure/postgres/phase-two-grants.psql` with the same database and migration owner selected through libpq. Apply `psql -X -v ON_ERROR_STOP=1 -1 -f ../infrastructure/postgres/registration-grants.psql` after the registration migration to grant execution of the fixed registration operation. Apply `psql -X -v ON_ERROR_STOP=1 -1 -f ../infrastructure/postgres/profile-grants.psql` after migration `202611250801` to grant execution of the fixed customer-profile operations; the runtime gets no table privilege on profile, preference or image rows. The runtime role `deypay_app` is created by the Phase 1 bootstrap. Its grants cover the repository projections and insertion of outbox and audit records. The SQLx event writer borrows the caller's database transaction. Launch the API and worker with their runtime login.

Migrations are forward-only. On a populated database, recover by a reviewed corrective migration or restore a verified backup into a separate database before switching traffic. A failed transactional migration can be fixed before first successful application. Preserve the contents and checksums of deployed migrations.

For the Rust integration suite, point `DATABASE_URL` at a dedicated disposable PostgreSQL instance with a database-creating test administrator and the restricted `deypay_app` role. From `backend/`, run `cargo test --locked -p deypay-infrastructure --features postgres-tests --test persistence`. SQLx creates a new database per test. Fixtures contain synthetic identities and asset precision. Production registry rows, address assignment policy, encryption/key management, retention policy and financial execution await their respective approvals.

## Sandbox registration

Use a local or test API and a disposable database. After migrating and applying the two runtime grant scripts above, run `psql -X -v ON_ERROR_STOP=1 -1 -f ../infrastructure/postgres/registration-sandbox.psql` as the migration owner from `backend/`. This selects the three customer-facing v1.0 drafts for that database. The application role can read the catalog and call the fixed registration operation.

Launch the API with its runtime database URL, `APP_ENV=local`, `DEYPAY_REGISTRATION_MODE=sandbox`, and these screening settings. Keep the fixture and settings outside Git.

| Variable | Sandbox value |
|---|---|
| `DEYPAY_SCREENING_FILE` | Local UTF-8 password-compromise dataset, one 64-hex-digit SHA-256 password digest per line. Use synthetic credentials in development. |
| `DEYPAY_SCREENING_SHA256` | Trusted SHA-256 of the complete file, supplied independently of the file. |
| `DEYPAY_SCREENING_VALID_UNTIL_UNIX` | Trusted expiry in Unix seconds. Missing, corrupt and expired datasets fail closed. |

Sandbox Argon2id defaults to 19,456 KiB, two iterations, one lane and two concurrent hashes. Operators can set `DEYPAY_ARGON2_MEMORY_KIB`, `DEYPAY_ARGON2_ITERATIONS`, `DEYPAY_ARGON2_LANES`, `DEYPAY_HASH_CONCURRENCY` and `DEYPAY_REGISTRATION_ATTEMPTS_PER_MINUTE` (default 30). The mode defaults to off. Staging and production keep registration disabled regardless of the sandbox settings.

Use `GET /api/v1/legal/documents` to discover the current versions and exact Markdown links, then submit the request in [API §13](docs/04-API-SPECIFICATION.md#13-user-registration-apis). To verify the packaged API, run `cargo test --locked --workspace --features postgres-tests` against disposable PostgreSQL with Node available. The SQLx suite starts the binary with an isolated sandbox dataset and exercises the production gate.

## Verification

From `backend/`, run `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo build --locked --workspace` and `cargo test --locked --workspace`. After building the binaries and starting the development database, run `node --env-file=.env --test scripts/backend-health.test.mjs` from the repository root. This covers API liveness and readiness, worker startup failure on a database outage and, on POSIX, graceful worker shutdown on SIGTERM. On Windows, stop the connected worker with Ctrl+C and check for the shutdown log and a zero exit code.

From `frontend/`, run `npm run lint`, `npm run build`, `npm run typecheck` and `npm test`. `npm test` runs the validation unit tests and starts the built frontend against an in-test HTTP double of the API to exercise the session guard, redirects, caching headers and unavailable states. From the root, run `node --test scripts/engineering-tracking.test.mjs scripts/local-environment.test.mjs` to verify the roadmap tracker, provider register and local credential setup.
## Email verification

The API and worker opt in with `EMAIL_VERIFICATION_ENABLED=true`. Registration keeps its own release gates. Local and test processes use sandbox consent records. Staging and production accept only identities with published registration evidence.

After applying the SQLx migrations, apply `infrastructure/postgres/email-verification-api-grants.psql` as the database owner alongside the existing API grants. Provision a dedicated least-privilege worker login, then apply `infrastructure/postgres/email-verification-worker-grants.psql` with `psql -v worker_role=<worker-login>` in the application database. Give each process its own `DATABASE_URL`. The worker grants allow four delivery operations. Only the API can consume a verification proof.

| Worker setting | Value |
|---|---|
| `EMAIL_PROVIDER` | `mock` in local/test, `resend` in staging/production. The mock retains the last message in process memory for tests. |
| `EMAIL_FROM` | `DEYPAY <deypay@kenuleafrica.xyz>` for Resend. |
| `RESEND_API_KEY` | Worker-only sending credential scoped to the verified sending domain. Supply through the deployment secret store. |
| `EMAIL_VERIFY_TOKEN_KEY` | A server-generated, cryptographically random 32-byte key encoded as 64 hex characters. Keep it stable across workers and restarts. For planned rotation, drain live deliveries and let outstanding links expire. |
| `EMAIL_VERIFY_BASE_URL` | The operator-approved absolute confirmation URL ending in `/verify-email`. HTTPS is required outside local/test. Local example: `http://127.0.0.1:8000/verify-email`. Route this path, `/verify-email.js` and the verification POST to the API on the same origin. |
| `EMAIL_VERIFY_TTL_SECONDS` | Default 900. The worker starts the expiry clock on first claim. |
| `EMAIL_TIMEOUT_SECONDS`, `EMAIL_MAX_ATTEMPTS`, `EMAIL_RETRY_BASE_SECONDS` | Defaults 10, 5 and 5. Backoff doubles to 900 seconds. A longer provider retry delay is respected while the link remains valid. |

API defaults are 60 seconds between requests per address (`EMAIL_RESEND_COOLDOWN_SECONDS`), five emails per address per hour including the initial intent (`EMAIL_RESEND_LIMIT_PER_HOUR`), five incorrect proofs per challenge (`EMAIL_VERIFY_MAX_GUESSES`) and 120 requests per endpoint per minute across replicas (`EMAIL_VERIFY_REQUESTS_PER_MINUTE`).

At deployment, verify the sender domain and leave link tracking disabled for verification mail. A provider acknowledgement records delivery submission. The only operation that verifies the address is the customer's confirmation POST. Monitor terminal `VERIFY_EMAIL` jobs by status and `last_error`. A customer can request a fresh message after a terminal failure and the resend cooldown.

## Password recovery and username availability

Migration `202611251301` adds password recovery (IMP-P3-008) and the username availability check (IMP-P3-009). Apply it after `202611251201`, then reapply `infrastructure/postgres/auth-grants.psql`. That script gives the verifier the two recovery capabilities and the IMP-P3-012 admission check, and it still verifies `auth_acl_ok()` and `auth_owner_ok()`. Apply `infrastructure/postgres/password-recovery-worker-grants.psql` with the same variables as the verification worker grants, and `infrastructure/postgres/username-availability-grants.psql` and `infrastructure/postgres/public-admission-grants.psql` for the general API. Configure `identity.admission_policy` rows for `forgot_password`, `reset_password` and `username_availability` (see `docs/engineering/AUTHENTICATION-RATE-LIMITS.md`). Without a row, that endpoint returns 503.

Route `POST /api/v1/auth/forgot-password` and `POST /api/v1/auth/reset-password` to the `deypay-auth` verifier at the ingress. The general API refuses both and serves only the static `/reset-password` page and `/reset-password.js`. Set `PASSWORD_RECOVERY_ENABLED=true` on the verifier and the worker; otherwise the verifier answers both routes with 503 and the worker sends no reset email.

| Setting | Process | Value |
|---|---|---|
| `DEYPAY_SCREENING_FILE`, `DEYPAY_SCREENING_SHA256`, `DEYPAY_SCREENING_VALID_UNTIL_UNIX` | Verifier | The compromised-password dataset used by registration. Required when recovery is enabled. |
| `PASSWORD_RESET_COOLDOWN_SECONDS`, `PASSWORD_RESET_LIMIT_PER_HOUR`, `PASSWORD_RESET_MAX_GUESSES` | Verifier | Defaults 60, 5 and 5, applied per account inside PostgreSQL. |
| `PASSWORD_RESET_TOKEN_KEY` | Worker | A random 32-byte key encoded as 64 hex characters, different from `EMAIL_VERIFY_TOKEN_KEY`. Supply through the secret store. |
| `PASSWORD_RESET_BASE_URL` | Worker | The approved absolute URL ending in `/reset-password` on the browser origin of the authentication release. HTTPS is required outside local/test. |
| `PASSWORD_RESET_TTL_SECONDS` | Worker | Default 1800 (owner decision, 2026-11-25), range 60–3600. |

The worker reuses `EMAIL_PROVIDER`, `EMAIL_FROM`, `RESEND_API_KEY` and the email retry settings. Monitor terminal `RESET_PASSWORD_EMAIL` jobs by status and `last_error`. Leave link tracking disabled for reset mail.

## Customer web app

`frontend/` serves the landing page, registration, email verification, sign-in, password recovery and the protected `/app` account area (overview, profile and photo, preferences, sessions). It calls only the implemented APIs; financial features are shown as unavailable.

- **One HTTPS origin.** Serve the web app and both APIs from the authentication release's browser origin (`DEYPAY_LOGIN_ORIGIN`). At the ingress, route `POST /api/v1/auth/{login,refresh,logout,forgot-password,reset-password}`, `GET /api/v1/auth/sessions` and `DELETE /api/v1/auth/sessions/{id}` to `deypay-auth`, other `/api/` paths to `deypay-api`, and everything else to the web app. The browser sends same-origin requests, so the session cookie stays host-only and the Origin check applies. Route `/verify-email` and `/reset-password` to the web app; its pages follow the same fragment-token handling as the API's fallback pages.
- **Server-side guard.** Every `/app` request forwards only the `__Host-deypay_session` cookie to `GET /api/v1/me` at `API_BASE_URL` (a private address; use TLS between hosts). A 401 redirects to sign-in, and nothing private renders or is cached.
- **Headers.** Middleware sets a per-request nonce Content-Security-Policy; all pages render dynamically so the nonce applies. Token pages send no referrer and are not stored.
- **Local development.** `next dev` proxies `/api/` to `API_BASE_URL` and the verifier paths to `DEYPAY_DEV_AUTH_URL` when both are set. The `__Host-` cookie and the Origin check need HTTPS, so use `next dev --experimental-https` with a matching `DEYPAY_LOGIN_ORIGIN` for live sign-in. Production builds never proxy.
