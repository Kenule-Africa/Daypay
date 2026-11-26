# DEYPAY codebase map

This is a navigation index, not an alternative specification. Verify source and
the applicable authoritative document before implementation.

## Backend workspace

Rust workspace: [`backend/Cargo.toml`](../../backend/Cargo.toml)

| Crate | Location | Responsibility |
|---|---|---|
| `deypay-domain` | `backend/crates/deypay-domain/` | Domain entities, types, and invariants |
| `deypay-application` | `backend/crates/deypay-application/` | Application services and use cases |
| `deypay-infrastructure` | `backend/crates/deypay-infrastructure/` | Persistence and external adapters |
| `deypay-api` | `backend/crates/deypay-api/` | HTTP/API entry point |
| `deypay-worker` | `backend/crates/deypay-worker/` | Background processing entry point |
| `deypay-common` | `backend/crates/deypay-common/` | Shared configuration and runtime utilities |

## Navigation

- API entry point and current health routes: `backend/crates/deypay-api/src/main.rs`
- Worker entry point: `backend/crates/deypay-worker/src/main.rs`
- Workstation-only verification outbox (Cargo example, local/test only): `backend/crates/deypay-worker/examples/local_verification_outbox.rs`; usage in [LOCAL-EMAIL-VERIFICATION.md](LOCAL-EMAIL-VERIFICATION.md).
- Email verification routes and confirmation page live in `deypay-api/src/email_verification.rs` and `verify_email.html`. Worker claims, protected proofs and delivery adapters live in `deypay-infrastructure/src/email_verification.rs` and `email_delivery.rs`. Migration `202611190002_email_verification.sql` binds outbox intents, leases and single-use confirmation to PostgreSQL.
- Domain root and accepted KYC case state machine: `backend/crates/deypay-domain/src/lib.rs` and `backend/crates/deypay-domain/src/kyc.rs`
- Domain projections and exact units: `backend/crates/deypay-domain/src/`; repository contracts: `backend/crates/deypay-application/src/repositories.rs`; SQLx implementations: `backend/crates/deypay-infrastructure/src/repositories.rs`.
- Transaction-bound outbox and audit writes: `backend/crates/deypay-application/src/events.rs` and `backend/crates/deypay-infrastructure/src/events.rs`.
- Application root: `backend/crates/deypay-application/src/lib.rs`
- KYC application service on accepted `main`: `backend/crates/deypay-application/src/kyc.rs` (IMP-P4-002, engineering-accepted); exports through `src/lib.rs`. Co-located tests cover customer ownership, first-case requests, transition validation, stale/concurrent changes and fail-closed requirements. The accepted main branch has no PostgreSQL KYC write adapter, document processing, audit write, provider/reviewer flow or financial eligibility implementation. PR #10 adds a persistence/API and authorization foundation, but it is pending merge below; see [PROGRESS.md](PROGRESS.md#2026-11-25-pr-9-engineering-acceptance-and-next-phase-3-cycle) and [BLOCKERS.md](BLOCKERS.md).
- Shared configuration/runtime: `backend/crates/deypay-common/src/`
- PostgreSQL pool and readiness query: `backend/crates/deypay-infrastructure/src/database.rs`
- Database migrations: `backend/migrations/`, embedded by `deypay-infrastructure`. SQLx integration cases are in `backend/crates/deypay-infrastructure/tests/`.
- Customer registration uses `backend/crates/deypay-api/src/registration.rs` for HTTP and `backend/crates/deypay-application/src/registration.rs` for enrollment. The legal registry is in `backend/migrations/202611190001_registration_legal.sql`. The current atomic registration write and email-only contact progression are in `backend/migrations/202611200001_mvp_contact_verification.sql`. `deypay-api/src/legal.rs` serves the customer documents. The sandbox is opt-in and production registration remains closed.
- Password enrollment and the read-only `CredentialVerifier` contract are in `backend/crates/deypay-application/src/passwords.rs` (IMP-P3-004, engineering-accepted). `backend/crates/deypay-infrastructure/src/password_hashing.rs` implements bounded Argon2id verification, dummy-hash rejection for missing/unusable records, shared concurrency admission and the `needs_rehash` signal; it does not perform rehash writes. `backend/crates/deypay-api/tests/registration_http.rs` verifies a stored registration credential and rejects a different password. See [P3-004 acceptance](PROGRESS.md#2026-11-24--imp-p3-004-engineering-acceptance).
- Login, session management, logout and session revocation (IMP-P3-005/006/007/011, Completed — Engineering Accepted) are in `backend/crates/deypay-api/src/login.rs`, `backend/crates/deypay-api/src/login/controls.rs`, `backend/crates/deypay-api/src/sessions.rs`, `backend/crates/deypay-api/src/bin/deypay-auth.rs`, `backend/crates/deypay-application/src/login.rs`, and `backend/crates/deypay-infrastructure/src/sessions.rs`. The general API validates protected requests; the isolated verifier process handles credential login, refresh, logout and session controls. Authentication policy/release migrations are `backend/migrations/202611250502_authentication_releases.sql` and `backend/migrations/202611250503_authentication_ownership.sql`; session-control persistence is `backend/migrations/202611250701_session_controls.sql`. Role setup/grants are in `infrastructure/postgres/auth-roles.psql` and `infrastructure/postgres/auth-grants.psql`. Login/session integration tests are in `backend/crates/deypay-api/tests/login_password.rs`, `backend/crates/deypay-api/tests/cases/login_sessions.rs` and `backend/crates/deypay-api/tests/cases/session_controls.rs`. PRs #9 and #11 are merged; see [PROGRESS.md](PROGRESS.md). No production release/policy is activated.
- Customer profile, preferences, status and profile-image handling (IMP-P3-010, Completed — Engineering Accepted) are in `backend/crates/deypay-api/src/profile.rs`, `backend/crates/deypay-application/src/profile.rs`, `backend/crates/deypay-infrastructure/src/profiles.rs` and `backend/crates/deypay-infrastructure/src/profile_images.rs`. Migration `backend/migrations/202611250801_customer_profile.sql` and grants `infrastructure/postgres/profile-grants.psql` protect persisted profile/image data. HTTP coverage is in `backend/crates/deypay-api/tests/cases/profile_http.rs`; persistence coverage is in `backend/crates/deypay-infrastructure/tests/persistence/profiles.rs`. PR #12 merged; API §15 documents the accepted contract. Image retention/erasure, load/animated-image and upload-timeout evidence remain follow-ups; see [PROGRESS.md](PROGRESS.md).
- KYC provider port: `backend/crates/deypay-application/src/kyc_provider.rs` (`IdentityVerificationProvider`, IMP-P4-003), exported from the application crate. The deterministic mock adapter is `backend/crates/deypay-infrastructure/src/kyc_provider_mock.rs` (`MockKycProvider`, IMP-P4-004), exported from the infrastructure crate and available only for local/test. Neither supplies real provider capabilities, webhook verification, persistence or KYC approval; see [PROGRESS.md](PROGRESS.md#2026-11-24--imp-p4-003-and-p4-004-engineering-acceptance) and [BLOCKERS.md](BLOCKERS.md).
- Financial ledger (IMP-P5-001–017, In review): exact units, precision, token scaling and journal validation in `backend/crates/deypay-domain/src/units.rs`, `money.rs` and `ledger.rs`; the ledger service, commands and fail-closed `FinancialGate` in `backend/crates/deypay-application/src/ledger.rs`; the PostgreSQL adapter in `backend/crates/deypay-infrastructure/src/ledger.rs`; schema and posting functions in `backend/migrations/202611260001_financial_ledger.sql`; roles and grants in `infrastructure/postgres/ledger-roles.psql` and `ledger-grants.psql`; financial-invariant and adversarial tests in `backend/crates/deypay-infrastructure/tests/ledger.rs`. The trust boundaries (capability activations, operator login bindings, the provider evidence role) and their review are described in `docs/engineering/PHASE5-LEDGER-SECURITY-REVIEW.md`.
- Other provider integrations/adapters: **Planned**
- Backend tests include Rust unit tests beside source, registration HTTP tests in `backend/crates/deypay-api/tests/registration_http.rs`, SQLx persistence tests in `backend/crates/deypay-infrastructure/tests/`, and live service checks in `scripts/backend-health.test.mjs`.

## Open pull requests — pending merge only

PR #9's authentication paths are part of the accepted `main` baseline. Only PR #10 remains open and unmerged.

### PR #10 — IMP-P4-005 / IMP-P4-006 / IMP-P4-007 foundations — pending merge and owner hold

- KYC API and tests: `backend/crates/deypay-api/src/kyc.rs`, `backend/crates/deypay-api/tests/kyc_http.rs`. The API depends on a `CustomerAuthenticator`; the current PR binary has no trusted authenticator integration and mounts the routes fail-closed. Accepted P3-005/006 identity is available on `main`, but PR #10 remains on hold and unmerged.
- KYC application and PostgreSQL persistence: `backend/crates/deypay-application/src/kyc.rs`, `backend/crates/deypay-infrastructure/src/kyc.rs`, migration `backend/migrations/202611250001_kyc_case_writes.sql`, and grants `infrastructure/postgres/kyc-grants.psql`.
- Restrictions and central financial authorization: `backend/crates/deypay-domain/src/restrictions.rs`, `backend/crates/deypay-application/src/restrictions.rs`, `backend/crates/deypay-infrastructure/src/restrictions.rs`, and `backend/crates/deypay-application/src/financial_authorization.rs`.
- Transaction-limit foundations: `backend/crates/deypay-domain/src/limits.rs`, `backend/crates/deypay-domain/src/limit_usage.rs`, `backend/crates/deypay-domain/src/operations.rs`, and `backend/crates/deypay-application/src/limits.rs`.
- PostgreSQL evidence: `backend/crates/deypay-infrastructure/tests/persistence/kyc_cases.rs`, `kyc_security.rs`, and `restrictions.rs`, plus the corresponding persistence test support.
- PR #10 also proposes an API §17.7 status mapping change in `docs/04-API-SPECIFICATION.md`; this change is unmerged. Its restriction/limit foundations fail closed, no financial operation calls them, and KYC verification implementation remains on owner hold.

- Frontend: `frontend/app/`; frontend checks: `frontend/tests/`
- Local PostgreSQL bootstrap and verification: `infrastructure/postgres/`
- CI workflow: `.github/workflows/phase-one.yml`
- CI/testing policy: all agents use GitHub Actions first for broad or expensive verification, verify the exact commit and required jobs, inspect failure logs, and keep local checks targeted. See [Test Strategy v1.1 §§41 and 51–54](../07-TEST-STRATEGY.md#41-automated-testing-and-cicd), especially [§51](../07-TEST-STRATEGY.md#51-mandatory-github-actions-execution-policy), [§52](../07-TEST-STRATEGY.md#52-agent-ci-monitoring-protocol), [§53](../07-TEST-STRATEGY.md#53-local-testing-and-environment-specific-acceptance), and [§54](../07-TEST-STRATEGY.md#54-ci-security-performance-and-maintenance-rules).

## Specification navigation

- Product behavior: [`docs/01-FRS.md`](../01-FRS.md)
- System boundaries: [`docs/02-SYSTEM-ARCHITECTURE.md`](../02-SYSTEM-ARCHITECTURE.md)
- Persistence: [`docs/03-DATABASE-DESIGN.md`](../03-DATABASE-DESIGN.md)
- Public API contracts: [`docs/04-API-SPECIFICATION.md`](../04-API-SPECIFICATION.md)
- Accounting and ledger invariants: [`docs/05-LEDGER-ACCOUNTING.md`](../05-LEDGER-ACCOUNTING.md)
- Security and compliance: [`docs/06-SECURITY-COMPLIANCE.md`](../06-SECURITY-COMPLIANCE.md)
- Verification strategy: [Test Strategy v1.1 — Approved 26 November 2026](../07-TEST-STRATEGY.md), especially §§41 and 51–54 for GitHub Actions–first verification, exact-commit CI monitoring, failure-log handling and minimal local testing.
- Delivery sequence and gates: [`docs/08-IMPLEMENTATION-PLAN.md`](../08-IMPLEMENTATION-PLAN.md)

Engineering state and navigation are maintained in [`docs/engineering/`](.).
