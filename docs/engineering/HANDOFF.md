# Engineering handoff

## Current state

- **Exact accepted baseline:** `37391e381b9bbf63a345e029800c03be3ad071cd` (PR #8 merge commit).
- **Latest successful verification:** [GitHub Actions run 45506566567](https://github.com/Kenule-Africa/Daypay/actions/runs/45506566567), on final implementation commit `678f4fbde0b99bde30192f3f3475b48f56571b45`; backend and frontend jobs both succeeded.
- **Completed phases:** Phase 1 — Project Foundation and Infrastructure; Phase 2 — Database and Domain Foundations.
- **IMP-P3-001:** Completed; the email-only MVP registration/contact-verification change is technically accepted.
- **IMP-P3-002:** In testing; hosted Resend staging delivery and confirmation remain open.
- **IMP-P3-003:** Deferred — Post-MVP.
- **IMP-P3-004:** Completed — Engineering Accepted. PR [#8](https://github.com/Kenule-Africa/Daypay/pull/8) merged at `37391e381b9bbf63a345e029800c03be3ad071cd`; CI run [45506566567](https://github.com/Kenule-Africa/Daypay/actions/runs/45506566567) passed on implementation commit `678f4fbde0b99bde30192f3f3475b48f56571b45`. The read-only verifier checks stored Argon2id credentials, safely rejects bad/missing/unusable records, shares bounded hashing capacity, and signals older supported profiles for future rehashing without writing. It adds no login, session or rehash operation. See [PROGRESS.md](PROGRESS.md#2026-11-24--imp-p3-004-engineering-acceptance).
- **IMP-P3-005 / IMP-P3-006:** Assigned / Not Started to Codex as one approved cycle on `feat/codex-imp-p3-005-006-login-sessions` in workspace `DeyPay-codex`, based on latest accepted `origin/main`. The tasks remain separate roadmap IDs and canonical Backlog until implementation begins.
- **IMP-P4-001:** Completed and engineering-accepted; PR [#5](https://github.com/Kenule-Africa/Daypay/pull/5).
- **IMP-P4-002:** Completed and engineering-accepted; Claude's PR [#6](https://github.com/Kenule-Africa/Daypay/pull/6) merged at `d22181cc5bad7f46a87330ac25d8389ac1631d18`. Final implementation commit `96aa8e5deb1c95b3ff3500e406a81cdd54157576` passed backend and frontend CI run [45487110380](https://github.com/Kenule-Africa/Daypay/actions/runs/45487110380).
- **IMP-P4-003 / IMP-P4-004:** Completed and engineering-accepted together in Claude's PR [#7](https://github.com/Kenule-Africa/Daypay/pull/7), merged at `b1b9e2b4879f9b8603df29437ee67eb5ca44df23`. Both implementation commits are included; backend and frontend CI passed on final commit `2f61d79b3206edb397fa6ad0d00d4b0076d43c97` in run [45499292388](https://github.com/Kenule-Africa/Daypay/actions/runs/45499292388).
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
- PR #7 added the provider-independent KYC interface and a local/test-only mock. No real-provider capability or production readiness was verified; see [PROGRESS.md](PROGRESS.md#2026-11-24--imp-p4-003-and-p4-004-engineering-acceptance).
- PR #8 added password verification only. The six-character minimum, production Argon2id/concurrency profile, screening-source approval, and login/session security controls remain open. Dummy hashing is a timing mitigation, not proof of uniform timing; password rehashing is not implemented.
- The prior Phase 2 native PostgreSQL and restricted-role evidence remains in [PROGRESS.md](PROGRESS.md).

## Remaining implementation follow-ups

- IMP-P4-002 provides application contracts and in-memory regression coverage only. PostgreSQL persistence/permissions/concurrency, approved submission requirements and timestamp semantics, case lifecycle, trusted session identity, audit behavior, status mapping and separate financial eligibility remain open in [BLOCKERS.md](BLOCKERS.md). The KYC provider/reviewer/expiry/failure/reason policies remain unresolved in [DECISIONS.md](DECISIONS.md).
- Deposit-address provisioning and customer ownership remain downstream of `IMP-P6-007`/`IMP-P6-008`, and `IMP-P9-001`/`IMP-P9-002`.
- Customer-isolation behavior must be re-verified through authenticated registration and wallet/deposit flows under `IMP-P3-001`–`IMP-P3-013`, `IMP-P6-004` and `IMP-P9-001`–`IMP-P9-004`.
- The email worker handles verification delivery. General provider operations, webhook handling and durable polling remain with `IMP-P8-001`–`IMP-P8-014`. Financial provider capabilities still require written evidence.
- Phase 0 approvals and live financial execution remain outside technical foundation completion.

## Next action

The latest accepted engineering work is `IMP-P3-004`, merged in PR [#8](https://github.com/Kenule-Africa/Daypay/pull/8) at `37391e381b9bbf63a345e029800c03be3ad071cd`. Backend and frontend CI passed on implementation commit `678f4fbde0b99bde30192f3f3475b48f56571b45` in run [45506566567](https://github.com/Kenule-Africa/Daypay/actions/runs/45506566567). This is technical acceptance only; it is not production authentication approval and does not complete Phase 3.

## Current assignments

- Codex: `IMP-P3-005` — Login and `IMP-P3-006` — Session Management, assigned / not started together on branch `feat/codex-imp-p3-005-006-login-sessions` in workspace `DeyPay-codex`. Reuse P3-004's accepted verifier and existing authentication/session persistence. The cycle excludes P3-007 logout, P3-008 password recovery, P3-011 as a separate revocation task, P3-012 general rate limiting beyond secure-login requirements, P3-013 MFA, and all Phase 4 work. Do not implement KYC or modify Claude-owned modules.
- Claude: `IMP-P4-005` — KYC API Endpoints; `IMP-P4-006` — Customer Restriction Engine; and `IMP-P4-007` — Transaction Limit Foundation, assigned / not started on branch `feat/claude-imp-p4-005-007-kyc-controls` in workspace `DeyPay-claude`. Claude's prerequisite scope includes the IMP-P4-002 PostgreSQL KYC write adapter, atomic first-case creation, conditional transitions and database-backed tests. Database/security review is required for least-privilege permissions and shared migration/grant changes. IMP-P4-002 remains completed and engineering-accepted; this is follow-up prerequisite work, not a status reversal.

Codex owns Phase 3; Claude owns Phase 4. Claude must consume trusted customer identity from Codex's assigned P3-005/P3-006 implementation and must not implement a separate login or session system. The [assignment register](TASKS.md#parallel-assignments) records both active workstreams. Keep KYC provider mapping, reviewer authorization, expiry, re-verification, provider failure handling and decision-reason policy unresolved until the accountable owners decide. Legal, Security and Compliance production gates remain open.

## Codex Phase 3 assignment — IMP-P3-005 / IMP-P3-006

The repository owner authorized both tasks as one cycle, retaining separate roadmap IDs. The assigned branch is `feat/codex-imp-p3-005-006-login-sessions` in workspace `DeyPay-codex`, based on latest accepted `origin/main`. Assignment status is Assigned / Not Started; this documentation records authorization but does not create the branch or begin implementation. Reuse the accepted read-only `CredentialVerifier` from `IMP-P3-004`. The current prerequisites include Phase 2 identity/password/session persistence and Phase 3 registration; the existing database design defines `identity.password_credentials` and `identity.sessions`.

Keep implementation boundaries explicit: the API layer owns approved login/session HTTP handling; application services own login outcomes, session lifecycle contracts and trusted customer context; infrastructure owns credential/session persistence and protected token handling; tests cover API behavior and PostgreSQL persistence/concurrency. Do not change Claude-owned KYC code or introduce unapproved shared API contracts, migrations or grants. Coordinate shared-file changes with the engineering coordinator and database/security owners. Do not fold rehash writes, password recovery, logout, standalone revocation, general rate limiting, MFA or KYC behavior into this cycle. Ensure authenticated customer identity is server-derived and available for Claude's assigned `IMP-P4-005`.

Follow API §14 and Security SEC-AUTH-004/006/008 and SEC-SES-001–006: apply the minimum rate limiting and abuse protections required for secure login; use generic credential failures, avoid unnecessary account enumeration, record required security events; create cryptographically unpredictable opaque server-side sessions, store only token hashes, enforce approved idle and absolute expiry, set Secure/HttpOnly/SameSite=Lax cookies, protect cookie-authenticated state changes against CSRF, rotate identifiers and invalidate prior identifiers atomically, and reject expired or revoked sessions. Do not implement logout or the separate session-revocation task.

Security/Product must still resolve production Argon2id cost and concurrency, password minimum and screening policy; login rate-limit/abuse thresholds and timing/enumeration acceptance; authentication audit requirements; configured idle/absolute session lifetimes; and any unclear account-status eligibility mapping. Dummy hashing is not proof of uniform timing. Implement only minimum safe boundaries where later-task controls remain outstanding, and do not claim those later tasks complete.

## Next Phase 4 task readiness

`IMP-P4-005` — KYC API Endpoints is assigned but remains gated from customer-facing implementation. Trusted login/session identity is not yet available; Codex's assigned IMP-P3-005/006 must be implemented and verified first. The KYC PostgreSQL write adapter and reviewed role permissions are absent; submission data/documents are not approved; case/API/account status mapping and resubmission policy remain unresolved; and appropriate compliance audit evidence is pending under IMP-P4-008. Do not expose routes that accept caller-supplied customer identity or imply successful persistence before these dependencies are met.

`IMP-P4-006` and `IMP-P4-007` may proceed with fail-closed foundational work where approved requirements are sufficient. They must not activate financial operations or treat provider outcomes as customer approval. Reassess P4-005 endpoint activation after the authentication/session, persistence, approved-requirements, status-mapping, resubmission and audit dependencies are satisfied. Claude must not implement authentication/session functionality; Codex owns the assigned Phase 3 login/session cycle. No additional task or agent assignment is implied.

Codex and Claude continue to use separate worktrees and assigned branches. Implementation agents must edit only their recorded modules, preserve migrations/API/security boundaries, and open PRs to `main`. Copilot coordinates; the repository owner reviews and merges.
