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

## Verification

From `backend/`, run `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo build --locked --workspace` and `cargo test --locked --workspace`. After building the binaries and starting the development database, run `node --env-file=.env --test scripts/backend-health.test.mjs` from the repository root.

From `frontend/`, run `npm run lint`, `npm run build`, `npm run typecheck` and `npm test`. The HTTP smoke test starts the built frontend with a controlled API to exercise available and unavailable states. From the root, run `node --test scripts/engineering-tracking.test.mjs scripts/local-environment.test.mjs` to verify the roadmap tracker, provider register and local credential setup.
