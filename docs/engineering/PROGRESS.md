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
