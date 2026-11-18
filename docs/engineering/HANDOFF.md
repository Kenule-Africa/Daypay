# Engineering handoff

## Current state

- **Phase:** 1 — Project Foundation and Infrastructure.
- **Active tasks:** `IMP-P1-002` and `IMP-P1-003` need locked Rust verification. `IMP-P1-004` is complete. See [TASKS.md](TASKS.md) for the remaining Phase 1 statuses.
- **Working tree:** Kickoff is committed at `83ee97f`. Branch `optimize-agent-context` is at `89691a8`. Phase 1 application, frontend, database, CI and tracking changes are uncommitted.

## Evidence to load

- Sources: `backend/Cargo.toml`, `backend/Cargo.lock`, `backend/crates/deypay-domain/Cargo.toml`, `backend/crates/deypay-domain/src/`, `backend/crates/deypay-application/Cargo.toml`, `backend/crates/deypay-application/src/`, `backend/crates/deypay-infrastructure/Cargo.toml`, `backend/crates/deypay-infrastructure/src/`, `backend/crates/deypay-api/Cargo.toml`, `backend/crates/deypay-api/src/`, `backend/crates/deypay-worker/Cargo.toml`, `backend/crates/deypay-worker/src/`, `backend/crates/deypay-common/Cargo.toml`, and `backend/crates/deypay-common/src/`.
- Requirements: [architecture §39](../02-SYSTEM-ARCHITECTURE.md), [test strategy](../07-TEST-STRATEGY.md), and roadmap Phase 1 §14 and §§37–39.
- Verification history: [PROGRESS.md](PROGRESS.md).
- Environment and release gates: [BLOCKERS.md](BLOCKERS.md).

## Last verified work

The current frontend passed lint, its production build, TypeScript and an HTTP smoke test with a controlled API. Rust workspace formatting passed. Three root tests passed for roadmap tracking, provider tracking and local credential initialization. See [PROGRESS.md](PROGRESS.md) for the limits of each check.

## Active blockers

The registry is unreachable and SQLx is absent from the local cache. The lockfile still describes the kickoff crates. This Windows environment also lacks a usable native linker and SDK. PostgreSQL and Docker runtime checks and a hosted CI run are outstanding. See [BLOCKERS.md](BLOCKERS.md).

## Next action

Use a development runner with registry access or a populated cache and a working Rust linker. From `backend/`, generate and review the dependency lockfile, then run:

```text
cargo generate-lockfile
cargo metadata --format-version 1 --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
```

Follow the [local setup](../../README.md) to start PostgreSQL and verify the restricted application role. Run `node --env-file=.env --test scripts/backend-health.test.mjs` from the root against the built API. Start the worker against the same database and confirm startup and shutdown. Record results here and in [PROGRESS.md](PROGRESS.md). The session instructions require explicit authorization before committing or pushing.
