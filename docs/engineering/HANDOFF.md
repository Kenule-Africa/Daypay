# Engineering handoff

## Current state

- **Verified commit:** `02fda26633b38946f1bb4d15d0ad86cee0a57656`.
- **Successful verification:** [GitHub Actions run 44344981832](https://github.com/Kenule-Africa/Daypay/actions/runs/44344981832).
- **Completed phases:** Phase 1 — Project Foundation and Infrastructure; Phase 2 — Database and Domain Foundations.
- **Current batch:** `IMP-P3-001` registration (Blocked) and `IMP-P3-009` username uniqueness (In testing), uncommitted against `c2fb966`.
- The public binary returns HTTP 503 for registration. In sandbox composition the four-field route requires trusted, submission-bound profile and consent evidence, applies password screening and acknowledges new registrations, replays and reserved names with HTTP 202.
- PostgreSQL writes the contact-unverified identity, canonical username, credential, profile, immutable consent, audit and two verification intents atomically. Email and phone delivery belong to the subsequent verification tasks.

## Verified evidence

- This Phase 3 working tree passed the locked build, all-target Clippy and 22 Rust tests. The live Windows smoke rerun passed all three applicable checks. Full commands, the earlier readiness failure and the platform skip are in [PROGRESS.md](PROGRESS.md).
- The earlier Phase 1 and 2 backend and frontend jobs passed on Ubuntu 24.04 at `02fda26`.
- Backend CI passed formatting, locked workspace Clippy, build, workspace tests, local environment/tracking checks, Compose PostgreSQL provisioning, API/worker health and outage tests, and the PostgreSQL persistence test suite.
- Frontend CI passed `npm ci`, lint, build, typecheck and Node tests.
- The lockfile consistency guard passed with the corrected `backend/Cargo.lock`.
- The prior Phase 2 native PostgreSQL and restricted-role evidence remains in [PROGRESS.md](PROGRESS.md).

## Remaining implementation follow-ups

- Deposit-address provisioning and customer ownership remain downstream of `IMP-P6-007`/`IMP-P6-008`, and `IMP-P9-001`/`IMP-P9-002`.
- Customer-isolation behavior must be re-verified through authenticated registration and wallet/deposit flows under `IMP-P3-001`–`IMP-P3-013`, `IMP-P6-004` and `IMP-P9-001`–`IMP-P9-004`.
- Provider operation, webhook and durable-job foundations still require the later framework tasks `IMP-P8-001`–`IMP-P8-014`; account-specific provider capability evidence remains unverified.
- Phase 0 approvals and live financial execution remain outside technical foundation completion.

## Next action

The local PostgreSQL and real Argon2id verification passed. Review the registration changes and supply the approved production reserved-name catalog. See [test evidence](PROGRESS.md) and [release gates](BLOCKERS.md). Product and Legal must reconcile capture of FRS §8 evidence with the four-field API. Security must approve the password, screening and abuse controls before public activation. The following roadmap feature is `IMP-P3-002` email verification.
