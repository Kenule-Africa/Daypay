# Engineering progress

## 2026-11-19 · Registration and legal evidence (IMP-P3-001)

Starting revision `8f125eb` contains the previous registration batch. This working tree replaces four-field registration with the approved eleven-field contract. The real sandbox binary accepts explicit Terms acceptance and Privacy acknowledgement with the configured screening dataset. DEYPAY retains customer identity and KYC ownership.

Migration `202611190001` preserves legacy consent rows and snapshots all four supplied v1.0 drafts as immutable UTF-8/LF content. Only DEP-LEG-001–003 are customer-facing. New registrations bind the customer, both affirmative actions, the Terms, Privacy and wording revisions, database time, channel and audit correlation in one transaction. Existing records receive no invented evidence. Production legal and security approvals are tracked in [BLOCKERS.md](BLOCKERS.md).

- Local Windows/MSVC and disposable PostgreSQL 18.7 passed `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets --features postgres-tests -- -D warnings`, and `cargo build --locked --workspace`.
- `cargo test --locked --workspace --features postgres-tests -- --test-threads=2` passed 28 tests: seven unit tests and 21 PostgreSQL tests. These include a real API process with the runtime database role and a checksum-validated screening file. The tests checked draft isolation, synthetic publication, exact document bytes, required fields, duplicate/concurrent claims, failure rollback and a populated legacy upgrade.
- All 13 registration HTTP and persistence tests passed again after the sandbox setup change, including a check that rerunning setup preserves the active legal release.
- `node --test scripts/backend-health.test.mjs` passed three live checks. Windows skipped its POSIX worker shutdown check. `node --test scripts/engineering-tracking.test.mjs scripts/local-environment.test.mjs` passed all three checks. This evidence is local to the working tree.

## 2026-11-18 · Phase 3 registration and usernames

Working tree based on `c2fb966`. `IMP-P3-001` remains Blocked and `IMP-P3-009` is In testing. The API and use case take no financial action and issue no session. Verification messages are queued as transactional intents. The [decision register](DECISIONS.md) records the four-field contract, uniform 202 response and provisional password policy.

- On a new loopback PostgreSQL 18.7 cluster, all eight ordered migrations applied with `psql -X -1 -v ON_ERROR_STOP=1`. With the restricted runtime grant, a new submission, normalized email replay, canonical username collision and reserved-name conflict produced one unverified identity and one consent record with the supplied timestamp. Two verification intents were present. Session, wallet and transaction counts were zero. An injected failure in the second intent rolled the new registration back.
- `cargo fmt --all -- --check` passed. Both `node --test scripts/engineering-tracking.test.mjs` checks passed. `cargo fetch --locked` succeeded after network access was enabled. On the working tree, `cargo test --locked --workspace --features postgres-tests -- --test-threads=2` passed all 22 tests. Six were unit tests and 16 used disposable PostgreSQL 18.7 (three HTTP registration, five registration persistence and eight existing persistence tests). HTTP tests used real salted Argon2id with test-only costs and a deterministic compromise verdict. Cases included stale consent, unavailable screening, canonical conflicts, simultaneous email and handle claims, no sessions or wallets, restricted-role grants and rollback. Locked Clippy with all workspace targets and the PostgreSQL test feature passed with warnings denied after correcting a byte-slice assertion in the Argon2id test. The corrected test also passed on a focused rerun. No CI run covers this working tree.

- `cargo build --locked --workspace` passed. Against a fresh cluster, `node --test scripts/backend-health.test.mjs` passed all three applicable live checks, including HTTP 503 for public registration, absence of a session cookie and absence of the submitted email and password in captured logs. The Windows runner skipped the POSIX worker shutdown test. An earlier run during compilation saw one unexpected 503 from database readiness. The standalone rerun passed with the database healthy and during the simulated outage.

## 2026-11-18 · Verified GitHub Actions run

Commit `02fda26633b38946f1bb4d15d0ad86cee0a57656` passed the [Phase 1 verification Actions run](https://github.com/Kenule-Africa/Daypay/actions/runs/44344981832).

| Task | Verified result |
|---|---|
| IMP-P1-002 | Passed. `backend/Cargo.lock` is committed at this revision and contains the resolved dependency graph. The backend job ran `cargo fetch` and locked Cargo commands successfully. |
| IMP-P1-003 | Passed. The backend job ran Rust formatting, locked workspace Clippy, workspace build and workspace tests successfully on Ubuntu 24.04. |
| IMP-P1-004 | Passed. The frontend job ran `npm ci`, lint, production build, typecheck and Node tests successfully. |
| IMP-P1-005 / 006 | Passed. The backend job parsed Compose configuration, started the disposable PostgreSQL service, waited for health and verified the runtime database login and privileges. |
| IMP-P1-007 | Passed. The backend workflow ran local credential initialization and environment tests. |
| IMP-P1-008 | Passed. The backend health suite ran the JSON API and worker processes, checking startup and credential leakage; the backend job succeeded. |
| IMP-P1-009 | Passed. The backend health suite exercised API liveness and database readiness, including available and unavailable database outcomes. |
| IMP-P1-010 | Passed. Both backend and frontend jobs succeeded for commit `02fda266`; the lockfile consistency guard passed. |
| IMP-P1-011 | Passed. Rust formatting and warnings-denied workspace Clippy passed in backend CI; frontend lint passed in frontend CI. |
| IMP-P1-012 | Passed. CI ran tracking/local-environment tests, disposable PostgreSQL checks, live API/worker outage tests and the infrastructure persistence test target. |
| IMP-P2-001–012 | Passed. The backend persistence step ran locked infrastructure Clippy and the `postgres-tests` persistence test suite against disposable PostgreSQL, in addition to the earlier native PostgreSQL evidence. |

The run emitted Node.js 20 deprecation warnings for `actions/checkout` and `actions/setup-node`; these warnings did not fail either job.

The corrected lockfile is an actual `02fda266` commit change. This run closes the resolved Cargo dependency, workspace, native-linking, lockfile and CI blockers for the recorded Phase 1/2 acceptance. It does not verify real provider capabilities, live KYC/provider integrations or production financial execution.

## Phase 2 · Persistence verification

2026-11-18 · IMP-P2-012. On fresh PostgreSQL 18.7 all seven migrations, three SQL constraint suites and runtime-role restrictions passed. All 11 parameterized repository SELECTs prepared and executed under that restricted role. rustc with MSVC ran the exact 38-digit boundary unit test (1 passed). Cargo formatting and both tracker tests passed. Locked Clippy, build and SQLx tests exited 101 resolving uncached uuid. The two-session PostgreSQL claim query passed natively. Executing its Rust test, the transaction-bound outbox/audit writer, the repository adapters and SQLx migration replay still requires a registry-enabled runner.

2026-11-18 · IMP-P2-008–011. Seven ordered migration files and all three SQL case files passed on a clean PostgreSQL 18.7 cluster. Cases cover scoped callbacks, forged-before-valid delivery, duplicate payout attempts and settlements, atomic rollback, job leases, audit immutability and maker-checker separation. Formatting and tracking tests passed. The same missing SQLx cache blocked offline lint, build and the Rust integration suite.

2026-11-18 · IMP-P2-005–007. On clean PostgreSQL 18.7, all five migration files and the identity and money/wallet cases passed. Verified 38-digit exact round-trip, rejection of fractional, oversized and NaN amounts, assetless fees, NGN wallet substitution, asset-unit changes, duplicate idempotency keys and history deletion. No assets, network routes or precision policies seeded. Formatting and tracker tests passed; locked Clippy/build/SQLx tests stopped at dependency resolution.

2026-11-18 · IMP-P2-002–004. Native PostgreSQL 18.7 ran in a fresh password-authenticated loopback cluster. psql applied migrations 001–003 in order and identity_constraints.sql passed (unique email/handle, device ownership, password hash and KYC state). Workspace formatting and both tracker tests passed; offline Clippy, build and SQLx test execution stopped at missing SQLx.

2026-11-18 · IMP-P2-001. Embedded ordered SQLx migrations and a clean-database/replay integration test. Workspace formatting and both tracking tests passed. Locked Clippy, build and SQLx tests stopped before compilation at unavailable SQLx. PostgreSQL 18 and MSVC are installed now. First disposable initdb timed out during file synchronization; retrying with a disposable cluster. No new CI result.

The primary checkout advanced to `5e633d9`, whose tracked tree matched the Phase 1 base `72db680`. The Phase 2 work was transferred to the primary checkout after checking for overlapping edits.

The Phase 1 [backend CI job at 72db680](https://github.com/Kenule-Africa/Daypay/actions/runs/44253202369/job/134628600410) passed its Rust, database, HTTP and worker lifecycle steps and failed the lockfile check. The full lockfile recovered from its diff hashes to Git blob `0a50b9a1169930302c88e6de00847b45d50b0e7e`. This graph predates Phase 2's UUID dependency. No Phase 2 test result is inferred from that run.

### Commands and remaining evidence

- After each group, `cargo fmt --all -- --check` and `node --test scripts/engineering-tracking.test.mjs` passed. Locked offline workspace build and Clippy (all targets/features, warnings denied) and `cargo test --locked --offline -p deypay-infrastructure --features postgres-tests --test persistence` stopped at dependency resolution. Online `cargo generate-lockfile` exited 101 reaching index.crates.io.
- For native tests, created a new PostgreSQL 18.7 data directory with `initdb --auth=scram-sha-256`, started `postgres` on an ephemeral loopback port, and used `psql -X -1 -v ON_ERROR_STOP=1 -f <file>` for each ordered migration and then each `tests/cases/*.sql` file. Created a temporary restricted role, applied `phase-two-grants.psql`, exercised `runtime-role.psql`, and prepared and executed all eleven repository SELECTs as that role. Two simultaneous sessions using the suite's `FOR UPDATE SKIP LOCKED` query selected different jobs. The test instance was stopped after each run.
- Verified workspace target discovery with `cargo metadata --no-deps --locked --offline`, all three tracker/environment tests, and the enforced LF attribute for migration files. `git diff --check` passed.
- In an MSVC developer environment, `rustc --edition 2024 --test backend/crates/deypay-domain/src/units.rs` produced a test executable. Running it passed the exact-unit range test. The new [SQLx tests](../../backend/crates/deypay-infrastructure/tests/persistence.rs) still require the Rust dependency graph and a disposable PostgreSQL administrator. No CI run is attributed to these uncommitted changes.

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
