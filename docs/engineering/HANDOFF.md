# Engineering handoff

## Current state

- **Phase 1 and 2 CI revision:** `02fda26633b38946f1bb4d15d0ad86cee0a57656`.
- **Successful verification:** [GitHub Actions run 44344981832](https://github.com/Kenule-Africa/Daypay/actions/runs/44344981832).
- **Completed phases:** Phase 1 — Project Foundation and Infrastructure; Phase 2 — Database and Domain Foundations.
- Registration and email verification are committed through `dfa9368`. Registration remains In review. Email verification remains In testing pending hosted delivery evidence.
- API §13 defines the eleven-field form and versioned legal links. Enable sandbox registration explicitly in both the database and local/test API. See [sandbox setup](../../README.md#sandbox-registration). Staging and production registration remain closed.
- MVP registration stores the required E.164 phone and queues one email verification intent. Email confirmation moves the customer to `CONTACT_VERIFIED`. The phone stays unverified. DEYPAY retains ownership of authentication and KYC, with separate eligibility and financial controls.

## Verified evidence

- The MVP contact change passed formatting, locked Clippy and build, all 36 Rust tests (27 on disposable PostgreSQL 18.7), and six Node checks on Windows. The populated upgrade preserved the historical phone intent and both proof timestamps. The POSIX worker SIGTERM check was skipped on Windows. [Verification record](PROGRESS.md)
- `IMP-P3-002` passed formatting, locked Clippy, build, all 35 Rust tests and six applicable Node checks locally. Twenty-six Rust tests used PostgreSQL 18.7. The worker SIGTERM smoke is POSIX-only. The task remains In testing pending a live staging send and confirmation. [Verification record](PROGRESS.md)

- The legal integration passed 28 Rust tests, including 21 on disposable PostgreSQL and an executable sandbox/production test. Formatting, locked Clippy, build and the applicable live smoke checks passed locally. See [current verification](PROGRESS.md).
- The preceding Phase 3 registration batch passed the locked build, all-target Clippy and 22 Rust tests. The live Windows smoke rerun passed all three applicable checks. Full commands, the earlier readiness failure and the platform skip are in [PROGRESS.md](PROGRESS.md).
- The earlier Phase 1 and 2 backend and frontend jobs passed on Ubuntu 24.04 at `02fda26`.
- Backend CI passed formatting, locked workspace Clippy, build, workspace tests, local environment/tracking checks, Compose PostgreSQL provisioning, API/worker health and outage tests, and the PostgreSQL persistence test suite.
- Frontend CI passed `npm ci`, lint, build, typecheck and Node tests.
- The lockfile consistency guard passed with the corrected `backend/Cargo.lock`.
- The prior Phase 2 native PostgreSQL and restricted-role evidence remains in [PROGRESS.md](PROGRESS.md).

## Remaining implementation follow-ups

- Deposit-address provisioning and customer ownership remain downstream of `IMP-P6-007`/`IMP-P6-008`, and `IMP-P9-001`/`IMP-P9-002`.
- Customer-isolation behavior must be re-verified through authenticated registration and wallet/deposit flows under `IMP-P3-001`–`IMP-P3-013`, `IMP-P6-004` and `IMP-P9-001`–`IMP-P9-004`.
- The email worker handles verification delivery. General provider operations, webhook handling and durable polling remain with `IMP-P8-001`–`IMP-P8-014`. Financial provider capabilities still require written evidence.
- Phase 0 approvals and live financial execution remain outside technical foundation completion.

## Next action

The next eligible roadmap task is `IMP-P3-004`, password hashing. Start from the existing registration Argon2id implementation and its tests. `IMP-P3-003` is Deferred — Post-MVP. Staging email delivery and production registration approvals remain open. See the [verification evidence](PROGRESS.md) and [release gates](BLOCKERS.md).
