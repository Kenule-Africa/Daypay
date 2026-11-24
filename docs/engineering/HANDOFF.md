# Engineering handoff

## Current state

- **Exact accepted baseline:** `b1b9e2b4879f9b8603df29437ee67eb5ca44df23` (PR #7 merge commit).
- **Latest successful verification:** [GitHub Actions run 45499292388](https://github.com/Kenule-Africa/Daypay/actions/runs/45499292388), on final implementation commit `2f61d79b3206edb397fa6ad0d00d4b0076d43c97`.
- **Completed phases:** Phase 1 — Project Foundation and Infrastructure; Phase 2 — Database and Domain Foundations.
- **IMP-P3-001:** Completed; the email-only MVP registration/contact-verification change is technically accepted.
- **IMP-P3-002:** In testing; hosted Resend staging delivery and confirmation remain open.
- **IMP-P3-003:** Deferred — Post-MVP.
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
- The prior Phase 2 native PostgreSQL and restricted-role evidence remains in [PROGRESS.md](PROGRESS.md).

## Remaining implementation follow-ups

- IMP-P4-002 provides application contracts and in-memory regression coverage only. PostgreSQL persistence/permissions/concurrency, approved submission requirements and timestamp semantics, case lifecycle, trusted session identity, audit behavior, status mapping and separate financial eligibility remain open in [BLOCKERS.md](BLOCKERS.md). The KYC provider/reviewer/expiry/failure/reason policies remain unresolved in [DECISIONS.md](DECISIONS.md).
- Deposit-address provisioning and customer ownership remain downstream of `IMP-P6-007`/`IMP-P6-008`, and `IMP-P9-001`/`IMP-P9-002`.
- Customer-isolation behavior must be re-verified through authenticated registration and wallet/deposit flows under `IMP-P3-001`–`IMP-P3-013`, `IMP-P6-004` and `IMP-P9-001`–`IMP-P9-004`.
- The email worker handles verification delivery. General provider operations, webhook handling and durable polling remain with `IMP-P8-001`–`IMP-P8-014`. Financial provider capabilities still require written evidence.
- Phase 0 approvals and live financial execution remain outside technical foundation completion.

## Next action

The latest accepted engineering work is `IMP-P4-003` and `IMP-P4-004`, merged together in PR [#7](https://github.com/Kenule-Africa/Daypay/pull/7) at `b1b9e2b4879f9b8603df29437ee67eb5ca44df23`. Backend and frontend CI passed on final implementation commit `2f61d79b3206edb397fa6ad0d00d4b0076d43c97` in run [45499292388](https://github.com/Kenule-Africa/Daypay/actions/runs/45499292388). This is technical acceptance only; Phase 4 is not complete and production gates remain open.

## Current assignments

- Codex: `IMP-P3-004` — Password Hashing, branch `feat/codex-imp-p3-004-password-hashing` (still pending).
- Claude: `IMP-P4-005` — KYC API Endpoints; `IMP-P4-006` — Customer Restriction Engine; and `IMP-P4-007` — Transaction Limit Foundation, assigned / not started on branch `feat/claude-imp-p4-005-007-kyc-controls` in workspace `DeyPay-claude`. Claude's prerequisite scope includes the IMP-P4-002 PostgreSQL KYC write adapter, atomic first-case creation, conditional transitions and database-backed tests. Database/security review is required for least-privilege permissions and shared migration/grant changes. IMP-P4-002 remains completed and engineering-accepted; this is follow-up prerequisite work, not a status reversal.

Claude must use trusted identity from Codex's Phase 3 authentication work and must not implement a separate login or session system. The [assignment register](TASKS.md#parallel-assignments) records each assigned task separately. Keep KYC provider mapping, reviewer authorization, expiry, re-verification, provider failure handling and decision-reason policy unresolved until the accountable owners decide. Legal, Security and Compliance production gates remain open.

## Next Phase 4 task readiness

`IMP-P4-005` — KYC API Endpoints is assigned but remains gated from customer-facing implementation. The API specification lists proposed KYC routes, but trusted login/session identity is not yet available (`IMP-P3-004`–`IMP-P3-006` remain in Codex's Phase 3 scope); the KYC PostgreSQL write adapter and reviewed role permissions are absent; submission data/documents are not approved; case/API/account status mapping and resubmission policy remain unresolved; and appropriate compliance audit evidence is pending under IMP-P4-008. Do not expose routes that accept caller-supplied customer identity or imply successful persistence before these dependencies are met.

`IMP-P4-006` and `IMP-P4-007` may proceed with fail-closed foundational work where approved requirements are sufficient. They must not activate financial operations or treat provider outcomes as customer approval. Reassess P4-005 endpoint activation after the authentication/session, persistence, approved-requirements, status-mapping, resubmission and audit dependencies are satisfied. Claude must not implement authentication/session functionality; Codex owns the Phase 3 identity work beginning with the existing IMP-P3-004 assignment. No additional task or agent assignment is implied.

Codex and Claude continue to use separate worktrees and assigned branches. Implementation agents must edit only their recorded modules, preserve migrations/API/security boundaries, and open PRs to `main`. Copilot coordinates; the repository owner reviews and merges.
