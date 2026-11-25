# Engineering handoff

## Current state

- **Latest accepted `main` baseline:** `502ac2dca4d99a684fbf158ca07349a81eaf5a27` (verified from GitHub on 2026-11-25). PR #9 is merged; PR #10 remains open, draft, unmerged and on hold.
- **Phase 1 and Phase 2:** accepted on the recorded evidence; the corrected Cargo.lock and hosted backend/frontend verification are recorded in [PROGRESS.md](PROGRESS.md#2026-11-18--verified-github-actions-run).
- **IMP-P3-001–004:** registration and password hashing retain their existing accepted/testing/deferred statuses. PR [#8](https://github.com/Kenule-Africa/Daypay/pull/8) remains the merged acceptance for P3-004.
- **IMP-P3-005 / IMP-P3-006:** Completed — Engineering Accepted from PR [#9](https://github.com/Kenule-Africa/Daypay/pull/9), merged at the current `main` SHA. Backend/frontend Actions run [45587411312](https://github.com/Kenule-Africa/Daypay/actions/runs/45587411312) succeeded. Hosted activation and policy approvals remain separate release gates in [BLOCKERS.md](BLOCKERS.md).
- **Next Phase 3 cycle — owner-authorized and scheduled, not started:** Codex owns IMP-P3-007 Logout and IMP-P3-011 Session Revocation on `feat/codex-imp-p3-007-011-session-controls` in `DeyPay-codex`; Claude owns IMP-P3-009 Username Uniqueness acceptance and IMP-P3-010 Customer Profile, including the approved profile-image extension, on `feat/claude-imp-p3-009-010-customer-profile` in `DeyPay-claude`. The repository owner creates/publishes the branches and worktrees from current `main`. See the file ownership and integration plan in [TASKS.md](TASKS.md).
- **Claude — IMP-P4-005 / IMP-P4-006 / IMP-P4-007:** Owner-directed hold. Draft PR [#10](https://github.com/Kenule-Africa/Daypay/pull/10) remains open and unmerged at head `a5ff8c3e1474f7bde4c4c06089cf10682f98c7cb` (10 commits, 27 files). Backend and frontend checks passed for the head in runs [45559944177](https://github.com/Kenule-Africa/Daypay/actions/runs/45559944177) and [45559952380](https://github.com/Kenule-Africa/Daypay/actions/runs/45559952380). The PR includes tested KYC persistence/API and fail-closed eligibility/limit foundations; these remain pending merge and are not accepted or available on `main`.
- **Accepted Phase 4 work:** IMP-P4-001–004 remain completed and engineering-accepted. PR #10's IMP-P4-002 persistence work is a follow-up, not a reopening of the accepted task.
- **Account status:** IMP-P3-002 remains In testing pending real staging delivery and confirmation; IMP-P3-003 remains Deferred — Post-MVP; IMP-P3-009 remains In testing until its assigned acceptance review is complete. IMP-P3-008, IMP-P3-012 and IMP-P3-013 remain unassigned. Profile-image contract details must be reviewed before image implementation.
- **Owner-directed delivery order:** complete customer-account backend, then the first real-API account frontend, then production-intended transaction implementation and internal verification, then resume KYC verification implementation. The scheduling exception and frontend milestone are recorded in [the roadmap](../08-IMPLEMENTATION-PLAN.md#30-phase-17--frontend-integration-and-user-experience).
- **KYC and financial eligibility:** application accounts and ordinary non-financial account management do not require KYC. KYC remains a separate eligibility gate for applicable financial services; no transaction may bypass KYC, account restrictions, limits, compliance or release approvals.
- **Release boundary:** CI success is not PR acceptance, production deployment, provider capability evidence, legal/regulatory approval or authorization for live financial execution.

## Verified evidence

- At baseline `d29fbfdea482c8ad7d3b4426a2440e0b6c95ccc3`, backend job `136317283655` and frontend job `136317283809` both succeeded. The backend run passed workspace formatting, locked Clippy/build/tests, disposable PostgreSQL, live HTTP/worker/outage and persistence checks. The frontend run passed npm install, lint, build, typecheck and Node tests.
- Registration and email-confirmation regressions passed, including required/malformed phones, email-only outbox intent, `CONTACT_VERIFIED` with unchanged phone verification, historical phone evidence and no session, wallet, transaction or KYC activation.
- `IMP-P3-002` remains In testing pending real staging delivery and confirmation. [Verification record](PROGRESS.md)

- The legal integration passed 28 Rust tests, including 21 on disposable PostgreSQL and an executable sandbox/production test. Formatting, locked Clippy, build and the applicable live smoke checks passed locally. See [current verification](PROGRESS.md).
- The preceding Phase 3 registration batch passed the locked build, all-target Clippy and 22 Rust tests. The live Windows smoke rerun passed all three applicable checks. Full commands, the earlier readiness failure and the platform skip are in [PROGRESS.md](PROGRESS.md).
- PR #9's merged implementation passed backend and frontend checks in Actions run [45587411312](https://github.com/Kenule-Africa/Daypay/actions/runs/45587411312); see the dated acceptance record in [PROGRESS.md](PROGRESS.md#2026-11-25-pr-9-engineering-acceptance-and-next-phase-3-cycle). Source review covered the two-process verifier boundary, release-bound login/session operations, database-backed status/expiry/revocation checks and the failure/concurrency integration cases.
- The earlier Phase 1 and 2 backend and frontend jobs passed on Ubuntu 24.04 at `02fda26`.
- Backend CI passed formatting, locked workspace Clippy, build, workspace tests, local environment/tracking checks, Compose PostgreSQL provisioning, API/worker health and outage tests, and the PostgreSQL persistence test suite.
- Frontend CI passed `npm ci`, lint, build, typecheck and Node tests.
- The lockfile consistency guard passed with the corrected `backend/Cargo.lock`.
- PR #7 added the provider-independent KYC interface and a local/test-only mock. No real-provider capability or production readiness was verified; see [PROGRESS.md](PROGRESS.md#2026-11-24--imp-p4-003-and-p4-004-engineering-acceptance).
- PR #8 added password verification only. The six-character minimum, production Argon2id/concurrency profile, screening-source approval, and hosted-login release approvals remain open; IMP-P3-005/006 themselves are engineering-accepted as recorded above. Dummy hashing is a timing mitigation, not proof of uniform timing; password rehashing is not implemented.
- The prior Phase 2 native PostgreSQL and restricted-role evidence remains in [PROGRESS.md](PROGRESS.md).

## Remaining implementation follow-ups

- Complete the authorized Phase 3 cycle within the [TASKS.md](TASKS.md) ownership matrix. For profile images, review the public API and production-intended private-storage contract before implementing image operations; current FRS §9.1/API §15 do not specify the contract.
- Deposit-address provisioning and customer ownership remain downstream of accepted authentication identity in IMP-P3-005/006, wallet-profile IMP-P6-004, deposit-address IMP-P6-007/008/009, and deposit-flow IMP-P9-001–004, with written provider-capability evidence required before live assignment.
- Verify customer isolation through authenticated registration, wallet, deposit and history flows under IMP-P3-001/005/006, IMP-P6-004, IMP-P9-001–004 and transaction-history IMP-P15-001–004.
- The email worker handles verification delivery. General provider operations, webhook handling, durable processing and status polling remain under IMP-P8-007/008–012/014; provider capability claims require written evidence.
- Further KYC verification work is on hold until the first account UI and internal transaction validation are complete and the owner authorizes resumption. This deferral does not waive KYC or other eligibility gates for financial services.
- Phase 0 approvals, security review, legal/compliance requirements, provider and partner approvals, production deployment and live financial execution remain release gates.

## Next actions

1. The repository owner creates/publishes the two authorized worktrees and branches from current `main`. Claude's P3 worktree must be distinct from the held PR #10 checkout; resolve the `DeyPay-claude` path collision before coding. Codex and Claude then implement only their assigned task scopes. Preserve the matrix and coordinate shared migrations, contracts and test fixtures.
2. Codex submits its IMP-P3-007/011 draft PR. Claude completes the assigned IMP-P3-009/010 work, reviews the profile-image API/storage contract before image implementation, and submits a separate draft PR. Integrate Codex's session-control PR first; Claude reconciles its branch after that merge, then reruns its tests.
3. Keep PR #10 open and on hold. Do not resume KYC verification implementation until the first account UI, internal transaction validation and additional owner authorization.
4. Keep IMP-P3-002 In testing, IMP-P3-003 deferred, and IMP-P3-008/012/013 unassigned. Do not claim branches or implementation are underway before the owner creates them.
5. Deliver the account-first frontend milestone in existing roadmap §30 only after account APIs are ready. It must use real APIs and must not invent balances or completed transactions.
6. Proceed to production-intended transaction implementation and internal verification only after the account UI milestone. Resume KYC verification implementation afterward, while keeping financial authorization fail-closed.

Codex owns the assigned authentication session-control work and Claude owns the assigned username/profile work; Claude's previous Phase 4 PR remains on hold. Preserve separate worktrees and migration/API coordination. The repository owner reviews and merges; this handoff grants no merge, deployment, provider or live-execution approval.
