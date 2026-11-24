# Engineering handoff

## Current state

- **Exact accepted baseline:** `d29fbfdea482c8ad7d3b4426a2440e0b6c95ccc3`.
- **Successful verification:** [GitHub Actions run 44744811033](https://github.com/Kenule-Africa/Daypay/actions/runs/44744811033).
- **Completed phases:** Phase 1 — Project Foundation and Infrastructure; Phase 2 — Database and Domain Foundations.
- **IMP-P3-001:** Completed; the email-only MVP registration/contact-verification change is technically accepted.
- **IMP-P3-002:** In testing; hosted Resend staging delivery and confirmation remain open.
- **IMP-P3-003:** Deferred — Post-MVP.
- API §13 defines the eleven-field form and versioned legal links. Sandbox registration remains explicitly enabled only for local/test. Staging and production registration remain closed.
- MVP registration requires and stores the E.164 phone and queues only email verification. Email confirmation moves the customer to `CONTACT_VERIFIED`, sets `email_verified_at`, and leaves `phone_verified_at` unchanged. DEYPAY retains authentication and KYC ownership with separate eligibility and financial gates.

## Verified evidence

- At baseline `d29fbfdea482c8ad7d3b4426a2440e0b6c95ccc3`, backend job `136317283655` and frontend job `136317283809` both succeeded. The backend run passed workspace formatting, locked Clippy/build/tests, disposable PostgreSQL, live HTTP/worker/outage and persistence checks. The frontend run passed npm install, lint, build, typecheck and Node tests.
- Registration and email-confirmation regressions passed, including required/malformed phones, email-only outbox intent, `CONTACT_VERIFIED` with unchanged phone verification, historical phone evidence and no session, wallet, transaction or KYC activation.
- `IMP-P3-002` remains In testing pending real staging delivery and confirmation. [Verification record](PROGRESS.md)

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

The latest accepted engineering work is `IMP-P4-001`, merged as PR [#5](https://github.com/Kenule-Africa/Daypay/pull/5) at `26d5023a02f0ed55fe3c95f1fb2b61394dd99c3f`, with passing backend and frontend CI in run [45467027795](https://github.com/Kenule-Africa/Daypay/actions/runs/45467027795). This is technical acceptance only.

## Next planned assignments

- Claude: `IMP-P4-002` — KYC Application Service, branch `feat/claude-imp-p4-002-kyc-application-service` (planned, not started). Read the unresolved policies in [DECISIONS.md](DECISIONS.md) and [BLOCKERS.md](BLOCKERS.md) before choosing behavior.
- Codex: `IMP-P3-004` — Password Hashing, branch `feat/codex-imp-p3-004-password-hashing` (still pending).

The [assignment register](TASKS.md#planned-parallel-assignments) records proposed ownership and dependencies. Keep KYC provider mapping, reviewer authorization, expiry, re-verification, provider failure handling and decision-reason policy unresolved until the accountable owners decide. Legal, Security and Compliance production gates remain open.

Codex and Claude continue to use separate worktrees and assigned branches. Implementation agents must edit only their recorded modules, preserve migrations/API/security boundaries, and open PRs to `main`. Copilot coordinates; the repository owner reviews and merges.
