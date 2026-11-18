# Engineering progress

## 2026-11-18 · Foundation kickoff

Git is already initialized on main. Commit `e96db69f5f3f4f4075469ca46d07808e18b98d16` contains all eight reconciled specifications. `git diff --exit-code HEAD -- docs` passed before kickoff. The existing untracked `prompt.md` supplied the session instructions.

| Task | Verified evidence |
|---|---|
| IMP-P0-001 | All eight specifications were reviewed and reconciled before kickoff. Their tracked versions match the baseline. |
| IMP-P0-002 | Reconciled IDs place the API at 004, ledger at 005, security at 006, tests at 007 and implementation at 008. |
| IMP-P0-005 | Reviewed the proposed custody boundary and recorded outstanding provider, key-control and safeguarding questions in DECISIONS.md. |
| IMP-P0-007 | Reviewed the reconciled API contract, including idempotency, proof binding and public states. See DECISIONS.md. |
| IMP-P0-008 | Created the scope-specific [provider register](PROVIDER-CAPABILITIES.md) covering all 20 written-evidence items in roadmap §42.2. All are unverified. |
| IMP-P0-009 | Identified legal, compliance and banking dependencies in DECISIONS.md with accountable functions and roadmap references. |
| IMP-P1-001 | Existing Git work tree, current branch and baseline contents verified. |

Formal approvals and phase exit criteria remain open. [TASKS.md](TASKS.md) records individual task status.

## 2026-11-18 · Rust workspace verification

The kickoff changes were subsequently committed at `83ee97f`. A clean working tree and matching local main and origin/main tracking refs were verified before Phase 1 continuation.

IMP-P1-002 is in testing. IMP-P1-003 has all six crate manifests and source roots and is blocked on native verification. The four libraries build offline. The API and worker have binary entry points that exit with a diagnostic until their services are implemented. Axum routes, the Tokio runtime, SQLx repositories and migrations belong to subsequent tasks.

| Verification | Result |
|---|---|
| Baseline, roadmap and links | All eight specs match HEAD. The tracking register matches all 220 IDs and titles, all 20 phases and the nine unnumbered Phase 17 sections. Tracking links resolve and all 40 FRS routes match the API. |
| Git exclusions | `git check-ignore` matched local secrets, dumps, target, node_modules and .next fixtures. Sample SQLx migration, .env.example and Cargo.lock paths remain eligible for version control. |
| `node --test scripts/engineering-tracking.test.mjs` | Both automated tests passed, including the 20 provider validation requirements in §42.2. |
| `cargo generate-lockfile --offline` and `cargo metadata --format-version 1 --locked --offline` | Lockfile generated. Metadata identifies four libraries and two binaries with publishing disabled. |
| `cargo check --workspace --all-targets --locked --offline` | Passed for all six crates, including test targets. |
| `cargo build --workspace --lib --locked --offline` | Passed for the four libraries. |
| Formatting | `cargo fmt --all -- --check` was blocked by Application Control. Direct `rustfmt --edition 2024 --check` passed on all six source files. |
| `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` | Application Control blocked clippy-driver.exe before linting. |
| `cargo test --workspace --locked --offline` | Failed at native linking. No Rust tests ran. |

The environment reported Git 2.56.0.windows.4, Rust/Cargo 1.100.0, Node 26.13.0 and npm 11.20.0. Rust is installed under the user profile and its bin directory was added to the verification process PATH. PostgreSQL client tools and Docker were unavailable on PATH, and TCP connection to 127.0.0.1:5432 was refused. See [BLOCKERS.md](BLOCKERS.md) for scope.

## 2026-11-18 · Resumed Phase 1 verification

Kickoff is committed on main at `83ee97f`. The working tree is based on `optimize-agent-context` at `89691a8`, which contains the updated repository instructions. The Phase 1 API, worker, frontend, database setup and CI changes are uncommitted. The earlier Cargo results above apply to the kickoff manifests.

| Task | Checks against the current working tree |
|---|---|
| IMP-P1-004 | Completed. In `frontend/`, `npm run build`, `npm run lint`, `npm run typecheck` and `npm test` passed. The smoke test started the production Next.js server and exercised its ready and unavailable UI states over HTTP with a controlled API. |
| IMP-P1-002 / 003 / 011 | `cargo fmt --all -- --check` passed. Locked offline metadata, Clippy, build and test commands exited 101 resolving SQLx. Online lockfile generation could not reach the registry. All four checks stopped before compilation. |
| IMP-P1-005 / 006 | Compose YAML parsed and its local mount paths resolved. Local PostgreSQL refused the connection on port 5432. The Docker CLI and psql were unavailable. Server startup and runtime privileges still require verification. |
| IMP-P1-007 / 008 / 009 | Rust checked the standalone configuration test module with `--emit=metadata`. API and worker execution, JSON logging and real health responses remain unverified. |
| IMP-P1-010 / 012 | Both tracking checks and the local credential-initialization regression passed. The regression checks distinct credentials, the restricted runtime login, preservation of an existing environment and console output for leaked credentials. Workflow YAML parsed. The backend HTTP suite passed its JavaScript syntax check. Its runtime checks and the hosted workflow run are pending. |

The current lockfile must be regenerated against the application dependencies before a locked build can verify Phase 1. See [HANDOFF.md](HANDOFF.md) for the next commands.

## Phase 1 acceptance at 8ec19a1

2026-11-18. Local HEAD and its remote-tracking ref are `8ec19a116f034e2f546f9e55227a3d96370fe088`. That commit contains the Phase 1 implementation. At session start the only modification was the owner's updated `docs/prompt.md`.

**Evidence sources.** Local results below were observed in this checkout. “Reported CI” refers to the owner's account of the latest Actions run in [the assignment](../prompt.md), interpreted against [the workflow](../../.github/workflows/phase-one.yml). The supplied report says the frontend job, Rust checks, PostgreSQL provisioning and HTTP suite passed before the Cargo.lock consistency check failed. The Actions log, tested SHA and run URL were unavailable for independent inspection. The new working-tree changes have no hosted run.

| Task | Verdict | Evidence and remaining action |
|---|---|---|
| IMP-P1-001 | Passed | Local Git history and branch refs confirm the repository and committed foundation. |
| IMP-P1-002 | Failed | The tracked lockfile lists only six local packages. Reported CI changed it during fetch. Recover the CI-generated lockfile or generate it with Cargo against crates.io, then verify full locked metadata. |
| IMP-P1-003 | Blocked | Local `cargo metadata --no-deps --locked` lists four libraries and two binaries. Reported CI built and tested its resolved graph. Build and test both binaries against the reviewed lockfile with the current shutdown changes. |
| IMP-P1-004 | Passed | Local frontend lint, production build, typecheck and HTTP smoke test passed. The built page displayed both expected states against a controlled API. The frontend CI job also passed as reported. |
| IMP-P1-005 | Passed (reported CI) | The provisioning step includes an authenticated TCP query that checks the database, runtime role, restricted privileges and server version. The local port refused connections. Retain the CI run reference with the final evidence. |
| IMP-P1-006 | Passed (reported CI) | The same step validates Compose and waits for the PostgreSQL container health check. Docker is unavailable on this Windows host. Retain the runner evidence. |
| IMP-P1-007 | Passed (local and reported CI) | Local credential initialization checks passed. Reported Rust tests cover configuration defaults and invalid settings, and HTTP tests launched the API with its runtime environment. Configuration and bootstrap source did not change in this session. |
| IMP-P1-008 | Blocked | Reported CI checked the API JSON startup message and absence of the connection URL. Rerun the expanded API/worker checks for JSON events and credentials in stdout and stderr. |
| IMP-P1-009 | Blocked | Reported CI exercised live HTTP 200 and readiness 200/503. Rerun the suite against the current API, which registers its shutdown handler before announcing startup. |
| IMP-P1-010 | Failed | Reported backend CI failed the reviewed-lockfile consistency check. Run both jobs against the corrected revision and record the green run and tested SHA. |
| IMP-P1-011 | Blocked | Local Rust formatting and frontend lint passed. Reported CI passed Clippy before the current runtime edits. Rerun locked Clippy with warnings denied. |
| IMP-P1-012 | Blocked | Three local tracking/credential tests and one frontend HTTP test passed. Reported CI passed both previous API/PostgreSQL cases. Run the expanded suite on Linux, including worker startup, SIGTERM exit and database outage. |

**Local blockers.** Online `cargo generate-lockfile` exited 101 connecting to index.crates.io. Locked offline metadata, Clippy, build and test each exited 101 resolving SQLx. Formatting passed. The expanded suite passed `node --check`; the worker outage case stopped at spawn with ENOENT. The environment has no built worker, Docker or psql. No worker behavior was observed locally.

**Phase gate.** Roadmap §14 requires a developer to run the full local application and its tests. Record a run with frontend, API, worker and PostgreSQL together, verify that the frontend shows the live API as available, and attach the locked CI result before opening Phase 2.
