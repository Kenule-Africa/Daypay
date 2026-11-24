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
- Email verification routes and confirmation page live in `deypay-api/src/email_verification.rs` and `verify_email.html`. Worker claims, protected proofs and delivery adapters live in `deypay-infrastructure/src/email_verification.rs` and `email_delivery.rs`. Migration `202611190002_email_verification.sql` binds outbox intents, leases and single-use confirmation to PostgreSQL.
- Domain root and accepted KYC case state machine: `backend/crates/deypay-domain/src/lib.rs` and `backend/crates/deypay-domain/src/kyc.rs`
- Domain projections and exact units: `backend/crates/deypay-domain/src/`; repository contracts: `backend/crates/deypay-application/src/repositories.rs`; SQLx implementations: `backend/crates/deypay-infrastructure/src/repositories.rs`.
- Transaction-bound outbox and audit writes: `backend/crates/deypay-application/src/events.rs` and `backend/crates/deypay-infrastructure/src/events.rs`.
- Application root: `backend/crates/deypay-application/src/lib.rs`
- KYC application service: `backend/crates/deypay-application/src/kyc.rs` (IMP-P4-002, engineering-accepted); exports through `src/lib.rs`. Co-located tests cover customer ownership, first-case requests, transition validation, stale/concurrent changes and fail-closed requirements. No PostgreSQL KYC write adapter, document processing, audit write, provider/reviewer flow or financial eligibility is implemented; see [PROGRESS.md](PROGRESS.md#2026-11-24--imp-p4-002-engineering-acceptance) and [BLOCKERS.md](BLOCKERS.md).
- Shared configuration/runtime: `backend/crates/deypay-common/src/`
- PostgreSQL pool and readiness query: `backend/crates/deypay-infrastructure/src/database.rs`
- Database migrations: `backend/migrations/`, embedded by `deypay-infrastructure`. SQLx integration cases are in `backend/crates/deypay-infrastructure/tests/`.
- Customer registration uses `backend/crates/deypay-api/src/registration.rs` for HTTP and `backend/crates/deypay-application/src/registration.rs` for enrollment. The legal registry is in `backend/migrations/202611190001_registration_legal.sql`. The current atomic registration write and email-only contact progression are in `backend/migrations/202611200001_mvp_contact_verification.sql`. `deypay-api/src/legal.rs` serves the customer documents. The sandbox is opt-in and production registration remains closed.
- Password enrollment and the read-only `CredentialVerifier` contract are in `backend/crates/deypay-application/src/passwords.rs` (IMP-P3-004, engineering-accepted). `backend/crates/deypay-infrastructure/src/password_hashing.rs` implements bounded Argon2id verification, dummy-hash rejection for missing/unusable records, shared concurrency admission and the `needs_rehash` signal; it does not perform rehash writes. `backend/crates/deypay-api/tests/registration_http.rs` verifies a stored registration credential and rejects a different password. No login or session implementation exists; see [PROGRESS.md](PROGRESS.md#2026-11-24--imp-p3-004-engineering-acceptance).
- KYC provider port: `backend/crates/deypay-application/src/kyc_provider.rs` (`IdentityVerificationProvider`, IMP-P4-003), exported from the application crate. The deterministic mock adapter is `backend/crates/deypay-infrastructure/src/kyc_provider_mock.rs` (`MockKycProvider`, IMP-P4-004), exported from the infrastructure crate and available only for local/test. Neither supplies real provider capabilities, webhook verification, persistence or KYC approval; see [PROGRESS.md](PROGRESS.md#2026-11-24--imp-p4-003-and-p4-004-engineering-acceptance) and [BLOCKERS.md](BLOCKERS.md).
- Other provider integrations/adapters: **Planned**
- Backend tests include Rust unit tests beside source, registration HTTP tests in `backend/crates/deypay-api/tests/registration_http.rs`, SQLx persistence tests in `backend/crates/deypay-infrastructure/tests/`, and live service checks in `scripts/backend-health.test.mjs`.
- Frontend: `frontend/app/`; frontend checks: `frontend/tests/`
- Local PostgreSQL bootstrap and verification: `infrastructure/postgres/`
- CI workflow: `.github/workflows/phase-one.yml`

## Specification navigation

- Product behavior: [`docs/01-FRS.md`](../01-FRS.md)
- System boundaries: [`docs/02-SYSTEM-ARCHITECTURE.md`](../02-SYSTEM-ARCHITECTURE.md)
- Persistence: [`docs/03-DATABASE-DESIGN.md`](../03-DATABASE-DESIGN.md)
- Public API contracts: [`docs/04-API-SPECIFICATION.md`](../04-API-SPECIFICATION.md)
- Accounting and ledger invariants: [`docs/05-LEDGER-ACCOUNTING.md`](../05-LEDGER-ACCOUNTING.md)
- Security and compliance: [`docs/06-SECURITY-COMPLIANCE.md`](../06-SECURITY-COMPLIANCE.md)
- Verification strategy: [`docs/07-TEST-STRATEGY.md`](../07-TEST-STRATEGY.md)
- Delivery sequence and gates: [`docs/08-IMPLEMENTATION-PLAN.md`](../08-IMPLEMENTATION-PLAN.md)

Engineering state and navigation are maintained in [`docs/engineering/`](.).
