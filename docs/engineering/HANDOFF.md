# Engineering handoff

## Current state

- **Phase:** 1 — Project Foundation and Infrastructure.
- **Active tasks:** `IMP-P1-002` and `IMP-P1-010` failed the lockfile gate. The [per-task acceptance record](PROGRESS.md#phase-1-acceptance-at-8ec19a1) covers all twelve tasks.
- **Working tree:** Branch `optimize-agent-context` is at `8ec19a1`, which contains the Phase 1 foundation. The owner's revised prompt and the current acceptance, signal handling and worker-test changes are uncommitted.

## Evidence to load

- Sources: `backend/Cargo.toml`, `backend/Cargo.lock`, `backend/crates/deypay-domain/Cargo.toml`, `backend/crates/deypay-domain/src/`, `backend/crates/deypay-application/Cargo.toml`, `backend/crates/deypay-application/src/`, `backend/crates/deypay-infrastructure/Cargo.toml`, `backend/crates/deypay-infrastructure/src/`, `backend/crates/deypay-api/Cargo.toml`, `backend/crates/deypay-api/src/`, `backend/crates/deypay-worker/Cargo.toml`, `backend/crates/deypay-worker/src/`, `backend/crates/deypay-common/Cargo.toml`, and `backend/crates/deypay-common/src/`.
- Requirements: [architecture §39](../02-SYSTEM-ARCHITECTURE.md), [test strategy](../07-TEST-STRATEGY.md), and roadmap Phase 1 §14 and §§37–39.
- Verification history: [PROGRESS.md](PROGRESS.md).
- Environment and release gates: [BLOCKERS.md](BLOCKERS.md).

## Last verified work

Local frontend lint, build, typecheck and HTTP smoke passed, as did Rust formatting, six-crate discovery and three root tests. The owner reports that CI passed PostgreSQL provisioning, Rust checks, API health/outage tests and the frontend job before the lockfile check failed. The previous run did not exercise the worker lifecycle. Its run URL, SHA and log have yet to be independently checked.

## Active blockers

The reviewed lockfile is stale. This Windows host cannot reach the registry, lacks cached SQLx and a usable MSVC build environment, and has no accessible PostgreSQL or Docker runtime. See [BLOCKERS.md](BLOCKERS.md).

## Next action

Recover the complete generated lockfile from the failed backend job's diff, or run `cargo generate-lockfile` from `backend/` on a trusted registry-enabled machine. Review the resolved dependencies. From `backend/`, run:

```text
cargo metadata --format-version 1 --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
```

Follow the [local setup](../../README.md) to start PostgreSQL and verify its restricted role. On Linux, run `node --env-file=.env --test scripts/backend-health.test.mjs` at the root. Require all four tests to pass, including worker shutdown on SIGTERM and rejection of an unreachable database. Run API, worker and frontend together against that database and verify the homepage shows “Available”. Once publication is authorized, record a green two-job Actions run, its SHA and the full-stack result in [PROGRESS.md](PROGRESS.md). Phase 2 awaits that gate. Committing, pushing or merging requires explicit authorization.
