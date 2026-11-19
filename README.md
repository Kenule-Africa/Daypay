# DEYPAY

The Phase 1 foundation contains a Rust API and worker, a Next.js PWA frontend foundation and a local PostgreSQL service. Delivery follows [the implementation roadmap](docs/08-IMPLEMENTATION-PLAN.md). Current verification evidence and outstanding setup requirements are in [the engineering handoff](docs/engineering/HANDOFF.md).

## Local development

Use stable Rust with rustfmt and Clippy, Node.js 22 LTS and PostgreSQL 16 or newer. On Windows, use an MSVC Developer PowerShell with its Windows SDK. Docker Compose v2 provides the containerized local database option.

1. From the repository root, run `node scripts/init-local.mjs` to create an ignored `.env` with two random local passwords. It preserves an existing file. The API and worker use only the runtime database login.
2. Start PostgreSQL with `docker compose up -d --wait postgres`. Data persists in a named development volume. The bootstrap creates a login with database connection and schema usage privileges on first initialization. Keep the generated credentials with that volume across restarts.
3. Verify the application login with `docker compose exec -T postgres sh -ec 'PGPASSWORD="$DEYPAY_DB_PASSWORD" psql -X -h 127.0.0.1 -U deypay_app -d deypay -f /opt/deypay/verify.psql'` from Bash or PowerShell. This checks the server version, database identity and privileges. For a native local instance, have its administrator create a fresh `deypay` database, then run `psql -X -d deypay -f infrastructure/postgres/bootstrap.psql` with the generated `DEYPAY_DB_PASSWORD` exported. Run `verify.psql` over TCP as `deypay_app`.
4. From separate terminals, run `node --env-file=.env scripts/run-local.mjs api` and `node --env-file=.env scripts/run-local.mjs worker`. The worker verifies its database connection at startup. Job handling belongs to later phases.
5. In `frontend/`, run `npm ci` and `npm run dev`. Open `http://127.0.0.1:3000`. The server renders readiness using the API at `http://127.0.0.1:8000`; set server-side `API_BASE_URL` when running the processes on different hosts.

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

Use PostgreSQL 16+ and SQLx CLI 0.8.6. From `backend/`, set `DATABASE_URL` in the migration process to the schema owner's database connection. Use verified TLS for hosted connections. Run `sqlx migrate run --source migrations`, then `sqlx migrate info --source migrations`. Reapplying the same release verifies checksums and skips applied versions. Run `psql -X -v ON_ERROR_STOP=1 -1 -f ../infrastructure/postgres/phase-two-grants.psql` with the same database and migration owner selected through libpq. Apply `psql -X -v ON_ERROR_STOP=1 -1 -f ../infrastructure/postgres/registration-grants.psql` after the registration migration to grant execution of the fixed registration operation. The runtime role `deypay_app` is created by the Phase 1 bootstrap. Its grants cover the repository projections and insertion of outbox and audit records. The SQLx event writer borrows the caller's database transaction. Launch the API and worker with their runtime login.

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

From `frontend/`, run `npm run lint`, `npm run build`, `npm run typecheck` and `npm test`. The HTTP smoke test starts the built frontend with a controlled API to exercise available and unavailable states. From the root, run `node --test scripts/engineering-tracking.test.mjs scripts/local-environment.test.mjs` to verify the roadmap tracker, provider register and local credential setup.
