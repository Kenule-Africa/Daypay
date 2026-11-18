# Engineering handoff

## Current state

- **Phase:** 1 — Project Foundation and Infrastructure.
- **Active tasks:** `IMP-P1-002` (in testing) and `IMP-P1-003` (blocked); statuses remain in [TASKS.md](TASKS.md).
- **Objective:** Finish native Rust workspace verification without changing the approved task register.
- **Working tree:** Contains uncommitted Phase 1 application, frontend, infrastructure, CI, and tracking changes. Inspect Git status and the diff before editing; do not treat them as committed evidence.

## Evidence to load

- Sources: `backend/Cargo.toml`, `backend/Cargo.lock`, `backend/crates/deypay-domain/Cargo.toml`, `backend/crates/deypay-domain/src/`, `backend/crates/deypay-application/Cargo.toml`, `backend/crates/deypay-application/src/`, `backend/crates/deypay-infrastructure/Cargo.toml`, `backend/crates/deypay-infrastructure/src/`, `backend/crates/deypay-api/Cargo.toml`, `backend/crates/deypay-api/src/`, `backend/crates/deypay-worker/Cargo.toml`, `backend/crates/deypay-worker/src/`, `backend/crates/deypay-common/Cargo.toml`, and `backend/crates/deypay-common/src/`.
- Requirements: [architecture §39](../02-SYSTEM-ARCHITECTURE.md), [test strategy](../07-TEST-STRATEGY.md), and roadmap Phase 1 §14 and §§37–39.
- Verification history: [PROGRESS.md](PROGRESS.md).
- Environment and release gates: [BLOCKERS.md](BLOCKERS.md).

## Last verified work

The six-crate workspace metadata, offline lockfile resolution, workspace check,
library build, and direct rustfmt checks were verified as recorded in
[PROGRESS.md](PROGRESS.md). Cargo test, Cargo fmt, and Clippy remain affected by
the native linker and Windows Application Control blockers; no unverified
success should be reported.

## Active blockers

Native Rust linking requires an MSVC linker and Windows SDK. Cargo fmt and
Clippy are blocked by Windows Application Control. Database integration checks
also require an accessible PostgreSQL service; details and owners are in
[BLOCKERS.md](BLOCKERS.md).

## Next action

Use a Windows Developer PowerShell with the MSVC linker and Windows SDK, or
another approved Rust build runner. Run from `backend/`:

```text
cargo metadata --format-version 1 --locked --offline
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo build --workspace --locked --offline
cargo test --workspace --locked --offline
```

Record commands and outcomes in [PROGRESS.md](PROGRESS.md) and update
[TASKS.md](TASKS.md) only when the roadmap acceptance evidence justifies a
status change. The next Phase 1 task after crate verification is `IMP-P1-004`;
local PostgreSQL setup is `IMP-P1-005`. Keep approvals, technical verification,
and live activation distinct.
