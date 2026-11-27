# Roadmap task register

[Document 08](../08-IMPLEMENTATION-PLAN.md) owns task definitions, sequence, phase deliverables and exit criteria. This register tracks its 220 task IDs. The section number on each phase locates the source. Phase 17 uses section references.

For backlog items, the roadmap task wording and phase objective describe the work. At selection, record the responsible owner, priority, related specification, dependencies, task-specific acceptance and test requirements in the active work record below. Use the status values in §37.3. Completed entries refer to dated verification in [PROGRESS.md](PROGRESS.md). Open decisions and external dependencies are in [DECISIONS.md](DECISIONS.md) and [BLOCKERS.md](BLOCKERS.md).

The product owner authorized eligible Phase 2 foundation work in parallel on 2026-11-18. GitHub Actions commit `02fda26633b38946f1bb4d15d0ad86cee0a57656` completed the resolved lockfile, backend and frontend verification on 2026-11-18.

## Active work record

For each phase, apply its exit criteria in Document 08, except for the repository owner's explicit scheduling exception recorded in §30. Record dated evidence for runtime behavior, unit and integration tests, applicable security controls and failure recovery. For financial work also verify ledger invariants, idempotency, concurrency and recovery. Identify the tested revision and environment, along with any gaps. Compilation alone cannot close a phase.

| Task | Owner / priority | Dependencies and requirement | Acceptance and verification |
|---|---|---|---|
| IMP-P1-002 | Backend engineering / P0 | Repository baseline, roadmap §§9–10, 14 | Cargo resolves the current workspace with its reviewed lockfile. |
| IMP-P1-003 | Backend engineering / P0 | IMP-P1-002; architecture §39 | Cargo metadata lists all six planned crates with API and worker binary targets. Format, lint and build/test the workspace and record the outcome. |
| IMP-P1-004 | Frontend engineering / P0 | Repository baseline, roadmap §14 | Build and start the Next.js app, check lint and types, and exercise the page over HTTP. See [PROGRESS.md](PROGRESS.md). |
| IMP-P1-005 / 006 | Infrastructure / P0 | Local database settings and PostgreSQL bootstrap, roadmap §14 | Start PostgreSQL with Compose and run the role, privilege and connection checks as the runtime login. |
| IMP-P1-007 / 008 / 009 | Backend engineering / P0 | Current workspace and PostgreSQL, roadmap §14 | Exercise configuration validation, inspect JSON logs for secret leakage, and verify API liveness and readiness with the database available and unavailable. |
| IMP-P1-010 / 011 / 012 | Engineering / P0 | Backend, frontend and development database, roadmap §14 | Run the workflow on GitHub and record the lint, formatting, build, unit and integration results. |

## Owner-approved execution schedule · 2026-11-25

This schedule cross-references existing phases, sections and task IDs only; it creates no task IDs or phases. Current status and owner-authorized assignments are stated explicitly below. Security verification is required during each applicable milestone, not deferred to Phase 18.

| Order | Existing roadmap scope | Dependency and milestone boundary | Security / release verification |
|---|---|---|---|
| 1 | Phase 3 customer-account backend: IMP-P3-001–013 | PR #14 merged to `main` at `4fc5c034385ebe34d3b4c9f10594664b923ecd50`; IMP-P3-008, P3-009 and P3-012 are Completed — Engineering Accepted on backend/frontend Actions run `45810331511`. IMP-P3-002 remains In testing pending real Resend staging delivery and confirmation; P3-003 remains Deferred — Post-MVP; P3-013 remains deferred by owner decision. | Actions acceptance verifies the exact merged commit only; live authentication remains blocked on production configuration, PostgreSQL connectivity, HTTPS routing and approved admission policies. Apply authentication and relevant IMP-P18 security checks during activation. |
| 2 | Early Phase 17 slice: IMP-P1-004; §30.1, §30.2, relevant §30.8 and §30.9 | After account APIs are ready: authentication screens, customer onboarding, dashboard shell and profile-management interfaces using the existing Next.js app and real APIs. This is not completion of Phase 17 and adds no task ID. | Verify session/cookie/CSRF behavior, authorization, privacy, input handling, loading/error states and real API integration during the slice. No mock product endpoints or invented financial data. |
| 3 | Phase 5–16 backend dependency chain: IMP-P5-001–017 → P6-001–010 → P7-001–010 → P8-001–014 → P9-001–010 and P10-001–012 → P11-001–010 → P12-001–009 and P13-001–010 → P14-001–014 → P15-001–010 → P16-001–012 | Follow the phase and task dependencies in Document 08; deposits and withdrawals follow the provider framework. IMP-P16-004 KYC review interface follows IMP-P4-009. | Apply relevant authorization, security, provider, concurrency, ledger and reconciliation checks from IMP-P18-001–008 and IMP-P18-012/013 as each capability is developed. |
| 4 | Internal transaction validation: IMP-P14-014 and applicable IMP-P18-002–008, IMP-P18-012/013 criteria | After the real transaction implementation is in place, test that same implementation under isolated conditions and authorized provider test environments. No substitute transaction engine or throwaway infrastructure. | Verify ledger invariants, idempotency, customer isolation, failure/recovery, reconciliation and fail-closed authorization before resuming deferred KYC implementation. |
| 5 | Resume Phase 4: IMP-P4-005–010; then dependent IMP-P16-004 | After internal transaction validation and additional owner authorization, complete applicable KYC verification, audit, review and provider work before live financial activation. IMP-P4-001–004 remain accepted; PR #10 code and existing task statuses remain unchanged until separately reviewed. | Keep live financial activity fail-closed on missing KYC, eligibility, restriction or policy evidence. Apply KYC and access-control security checks during implementation. |
| 6 | External release: remaining IMP-P18-001–013 and IMP-P19-001–012, plus applicable Phase 0 approvals | Close the cumulative security/quality exit criteria and production preparation only after implementation, internal validation and required KYC work. | Obtain applicable legal, regulatory, Compliance, provider, partner, operational and reconciliation approvals before live customer financial services. |

## Parallel assignments

Current implementation assignments are listed here. Assignment status is separate from the canonical roadmap task status; completed assignments and their acceptance evidence are recorded in [PROGRESS.md](PROGRESS.md).

| Agent | Task | Branch | Owned modules/files | Dependencies | Status | PR / acceptance evidence |
|---|---|---|---|---|---|---|
| Codex | IMP-P3-012 Authentication Rate Limiting | `feat/codex-imp-p3-012-auth-rate-limiting` | Implemented and merged in PR #14 at `4fc5c034385ebe34d3b4c9f10594664b923ecd50`. The assigned rate-limiting feature is engineering-accepted from the successful backend/frontend Actions run `45810331511`. Codex's next responsibility is authentication environment activation, subject to the blockers in [BLOCKERS.md](BLOCKERS.md); do not treat CI as live activation evidence. | Accepted identity/authentication interfaces; production deployment configuration, database connectivity, HTTPS routing and approved admission policies | Completed — Engineering Accepted; activation follow-up remains | PR #14 merged; exact-commit backend and frontend checks passed. Live environment and policy evidence remain outstanding. |
| Claude | IMP-P3-008 Password Recovery | `feat/claude-imp-p3-002-008-009-auth-completion` | Implemented and merged in PR #14 at `4fc5c034385ebe34d3b4c9f10594664b923ecd50`; backend and frontend Actions run `45810331511` passed for the exact merged commit. | IMP-P3-001/004/005/006; existing email delivery; IMP-P3-012 shared rate-limit interface; applicable API/security criteria | Completed — Engineering Accepted | PR #14 merged; deployment configuration and live service activation are separate gates. |
| Claude | IMP-P3-009 Username Availability endpoint | `feat/claude-imp-p3-002-008-009-auth-completion` | Implemented and merged in PR #14 at `4fc5c034385ebe34d3b4c9f10594664b923ecd50`; backend and frontend Actions run `45810331511` passed for the exact merged commit. Preserves prior uniqueness controls and uses the shared authentication rate-limit interface. | Accepted uniqueness implementation; API §16; IMP-P3-012 shared interface; approved username-enumeration policy | Completed — Engineering Accepted | PR #14 merged; backend/frontend CI does not establish live deployment or policy activation. |
| Claude | IMP-P3-002 Email Verification acceptance testing | `feat/claude-imp-p3-002-008-009-auth-completion` | Code and automated checks remain accepted as previously recorded. Real Resend staging delivery and confirmation are still outstanding; keep the canonical task In testing until actual authorized staging evidence is recorded. Do not substitute a mock delivery or claim external delivery from CI. | Existing verification implementation; approved staging origin and Resend configuration; authorized test recipient | In testing — staging acceptance pending | Record delivery and confirmation evidence in PROGRESS when performed; deployment and provider gates remain in [BLOCKERS.md](BLOCKERS.md). |
| Claude | IMP-P4-005 KYC API Endpoints | `feat/claude-imp-p4-005-007-kyc-controls` | Workspace: `DeyPay-claude`. PR implementation uses a `CustomerAuthenticator` port and server-derived identity; the current API binary has no trusted authenticator and KYC routes fail closed. It includes the IMP-P4-002 PostgreSQL follow-up. Further KYC verification implementation is held by owner decision. Preserve PR gaps; do not assume approved submission/document requirements. No login/session implementation. | Trusted P3-005/006 identity for any future operational API; PR10 KYC persistence and reviewed permissions; approved submission requirements and applicable compliance policy before resumption | On Hold — Owner-Directed | Draft, open and unmerged PR [#10](https://github.com/Kenule-Africa/Daypay/pull/10), head `a5ff8c3e1474f7bde4c4c06089cf10682f98c7cb`; backend/frontend CI passed in runs 45559944177 and 45559952380. Not accepted. |
| Claude | IMP-P4-006 Customer Restriction Engine | `feat/claude-imp-p4-005-007-kyc-controls` | Workspace: `DeyPay-claude`. PR contains fail-closed restriction and central authorization foundations. No financial operation is wired or activated. Preserve code and defer further work under the owner-directed hold. | IMP-P4-001–002; approved eligibility/restriction rules before any operational use; financial work must remain blocked unless eligibility is verified | On Hold — Owner-Directed | Same draft, open and unmerged PR [#10](https://github.com/Kenule-Africa/Daypay/pull/10), head `a5ff8c3e1474f7bde4c4c06089cf10682f98c7cb`; CI passed. Not accepted. |
| Claude | IMP-P4-007 Transaction Limit Foundation | `feat/claude-imp-p4-005-007-kyc-controls` | Workspace: `DeyPay-claude`. PR contains exact-money and limit-usage decision foundations; no thresholds are configured, usage is not persisted/consumed, and no financial operation calls them. Preserve code and defer further work under the owner-directed hold. | Approved limit thresholds, scopes/windows and eligibility requirements; coordinated implementation with IMP-P4-006 and transaction acceptance flows | On Hold — Owner-Directed | Same draft, open and unmerged PR [#10](https://github.com/Kenule-Africa/Daypay/pull/10), head `a5ff8c3e1474f7bde4c4c06089cf10682f98c7cb`; CI passed. Not accepted. |

The canonical roadmap statuses are separate from assignment status. IMP-P3-007/011 are Completed — Engineering Accepted through merged PR #11; IMP-P3-010 through merged PR #12; and IMP-P3-008/009/012 through merged PR #14 at `4fc5c034385ebe34d3b4c9f10594664b923ecd50`, with backend/frontend Actions run `45810331511` passing. IMP-P3-002 remains In testing until real Resend staging delivery and confirmation are evidenced. Next planned work is Claude's production account frontend and Codex's authentication environment activation; these are priorities, not new roadmap task IDs. Live activation remains subject to [BLOCKERS.md](BLOCKERS.md). IMP-P3-013 remains deferred by owner decision; P3-003 remains Deferred — Post-MVP. IMP-P4-005/006/007 remain blocked by the owner-directed hold while PR #10 remains open and unmerged.

## Shared-file ownership and integration plan

The agents work in parallel only within these boundaries. A file has one primary editor for this cycle; requests to change another owner's file must be coordinated before editing. Branches are owner-created from the verified `main` SHA above.

| Shared area | Primary editor | Boundary during parallel work |
|---|---|---|
| `backend/crates/deypay-api/src/main.rs` | Claude | Wire only Claude-owned recovery and username routes. Codex provides the stable rate-limit interface and must coordinate before any router/middleware edit. |
| `backend/crates/deypay-api/src/lib.rs` | Claude | Export Claude-owned recovery and username modules. Codex does not edit without prior coordination. |
| `backend/crates/deypay-api/Cargo.toml` | Codex | Own dependencies required by the shared rate-limit implementation; Claude coordinates any additional dependency changes before editing. |
| `backend/crates/deypay-application/src/lib.rs` | Codex | Own exports for the shared rate-limit interface. Claude coordinates recovery/username exports if they must be added to this same crate root. |
| `backend/crates/deypay-infrastructure/src/lib.rs` | Codex | Own rate-limit adapter exports and implementation wiring. Claude does not edit without prior coordination. |
| Shared PostgreSQL identity/authentication tables | Codex for any rate-limit schema change; Claude for any recovery-token schema change | Keep rate-limit and recovery records in separate task-owned schema changes if persistence is required. Do not alter username uniqueness or accepted session/account columns. Coordinate any shared-table change before implementation. |
| Migration filenames and global ordering | Coordinator-reserved, one unique forward-only migration per owner | Reserve unique identifiers/order before creation. Codex owns rate-limit migration contents; Claude owns any recovery migration contents. Never edit merged migrations or reuse identifiers. |
| Common session middleware and authenticated-principal contract | Codex | Rate-limit work may provide a shared integration interface but must not silently change the accepted identity/session trust boundary. Claude consumes accepted auth interfaces and does not modify their internals. |
| Shared Rust integration-test fixtures | Codex for rate-limit fixtures | Claude adds endpoint-specific tests and uses existing fixture APIs; coordinate and serialize any shared fixture change before editing. |

Integration order: both Cycle 2 branches start from accepted `main` `142dd04b3434327219e9a1c86603987aee3f798a` in separate worktrees. Their refs were not present in the verified GitHub branch listing; owner publication is still required. Codex first documents and implements the shared rate-limit interface and submits its draft PR. Claude may progress independently on P3-002 staging acceptance and isolated recovery/username modules, but dependent endpoints must consume the agreed interface. Integrate the rate-limit PR before completing and accepting the dependent endpoint PR; reconcile shared wiring and rerun tests. Claude's worktree must remain distinct from the held PR #10 checkout. Neither agent overwrites another's files; no merge or acceptance is implied by this schedule.

Additional coordination boundaries:

- Codex owns only the shared authentication rate-limiting scope and interface. It must not implement recovery/username business endpoints or modify profile, MFA, KYC or financial behavior.
- Claude must not modify rate-limiting implementation/interface internals, accepted verifier/session behavior, PR #10 or its suspended KYC branch, account eligibility policy, or financial operations. Recovery and username routes must consume the Codex-owned interface.
- Any overlapping router, crate-root, manifest, migration or shared-fixture edit requires prior coordination; no agent may assume the other will reconcile an unannounced change. Neither agent may introduce unapproved rate-limit, recovery or username policy.
- `AGENTS.md`, this task register, [HANDOFF.md](HANDOFF.md), progress evidence and decision/blocker records are Copilot-coordinator-owned. Implementation agents do not update tracker records unless specifically requested and approved.
- Parallel SQLx/PostgreSQL tests must use existing disposable test patterns. Each worktree should use its own environment file and isolated test database selection; do not start or modify permanent database services.

## Phase 0 · REQUIREMENTS AND ARCHITECTURE BASELINE (§13)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P0-001 | Review all eight core documents. | Completed |
| IMP-P0-002 | Resolve inconsistent document numbering and cross-references. | Completed |
| IMP-P0-003 | Confirm MVP scope. | In review |
| IMP-P0-004 | Confirm the Model B NGN approach. | In review |
| IMP-P0-005 | Review the custody model. | Completed |
| IMP-P0-006 | Confirm financial ledger posting rules. | In review |
| IMP-P0-007 | Review API contracts. | Completed |
| IMP-P0-008 | Create the provider capability register. | Completed |
| IMP-P0-009 | Identify legal and regulatory dependencies. | Completed |
| IMP-P0-010 | Approve implementation priorities. | In review |

## Phase 1 · PROJECT FOUNDATION AND INFRASTRUCTURE (§14)

The [acceptance results](PROGRESS.md#phase-1-acceptance-at-8ec19a1) give the prior verdict and evidence source for each ID. Commit `02fda26633b38946f1bb4d15d0ad86cee0a57656` passed the successful backend and frontend Actions run linked in [PROGRESS.md](PROGRESS.md).

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P1-001 | Initialize repository. | Completed |
| IMP-P1-002 | Create Rust workspace. | Completed |
| IMP-P1-003 | Create backend crates. | Completed |
| IMP-P1-004 | Initialize Next.js frontend. | Completed |
| IMP-P1-005 | Configure PostgreSQL. | Completed |
| IMP-P1-006 | Configure Docker development services. | Completed |
| IMP-P1-007 | Implement application configuration. | Completed |
| IMP-P1-008 | Implement structured logging. | Completed |
| IMP-P1-009 | Create health endpoints. | Completed |
| IMP-P1-010 | Establish CI pipeline. | Completed |
| IMP-P1-011 | Configure linting and formatting. | Completed |
| IMP-P1-012 | Create initial integration test infrastructure. | Completed |

## Phase 2 · DATABASE AND DOMAIN FOUNDATIONS (§15)

Backend engineering owns these P0 foundations. Task 001 supplies ordered migrations. Tasks 002–004 establish identity and KYC, 005–007 add the asset, wallet and transaction relationships, and 008–011 add provider, event and audit persistence. Task 012 uses those structures through application-owned contracts. Acceptance requires PostgreSQL constraints, role checks, migration replay and Rust adapter tests. See [Phase 2 evidence](PROGRESS.md#phase-2--persistence-verification).

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P2-001 | Create database migration framework. | Completed |
| IMP-P2-002 | Implement customer identity tables. | Completed |
| IMP-P2-003 | Implement authentication tables. | Completed |
| IMP-P2-004 | Implement KYC tables. | Completed |
| IMP-P2-005 | Implement asset registry. | Completed |
| IMP-P2-006 | Implement wallet profiles. | Completed |
| IMP-P2-007 | Implement transaction records. | Completed |
| IMP-P2-008 | Implement provider operation records. | Completed |
| IMP-P2-009 | Implement webhook inbox. | Completed |
| IMP-P2-010 | Implement outbox and durable jobs. | Completed |
| IMP-P2-011 | Implement audit tables. | Completed |
| IMP-P2-012 | Implement repository interfaces. | Completed |

## Phase 3 · AUTHENTICATION AND CUSTOMER IDENTITY (§16)

`IMP-P3-004` is Completed — Engineering Accepted. PR [#8](https://github.com/Kenule-Africa/Daypay/pull/8) merged into `main` at `37391e381b9bbf63a345e029800c03be3ad071cd`; backend and frontend CI passed in run [45506566567](https://github.com/Kenule-Africa/Daypay/actions/runs/45506566567) for implementation commit `678f4fbde0b99bde30192f3f3475b48f56571b45`. This acceptance does not approve production authentication. See [PROGRESS.md](PROGRESS.md#2026-11-24--imp-p3-004-engineering-acceptance).

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P3-001 | Registration. | Completed |
| IMP-P3-002 | Email verification. | In testing |
| IMP-P3-003 | Phone verification. | Deferred — Post-MVP |
| IMP-P3-004 | Password hashing. | Completed |
| IMP-P3-005 | Login. | Completed |
| IMP-P3-006 | Session management. | Completed |
| IMP-P3-007 | Logout. | Completed |
| IMP-P3-008 | Password recovery. | Completed |
| IMP-P3-009 | Username uniqueness. | Completed |
| IMP-P3-010 | Customer profile. | Completed |
| IMP-P3-011 | Session revocation. | Completed |
| IMP-P3-012 | Authentication rate limiting. | Completed |
| IMP-P3-013 | MFA and step-up authentication foundations. | Backlog |

IMP-P3-005/006 are Completed — Engineering Accepted from merged PR #9 at `502ac2dca4d99a684fbf158ca07349a81eaf5a27`; hosted backend/frontend CI passed in run `45587411312`. IMP-P3-007/011 are Completed — Engineering Accepted from merged PR #11; final implementation head `04ef4e62386de8747a1bb792723e4c04ebc515c5` passed backend/frontend Actions run `45598725222`. IMP-P3-010 is Completed — Engineering Accepted from merged PR #12; final head `0a61dcb48f3161cceb6b1560f0b7700116a5be2c` passed backend/frontend Actions runs `45629571933` and `45630944319`. Evidence and scope follow-ups are recorded in [PROGRESS.md](PROGRESS.md). These acceptances do not approve production activation. Claude's PR #10 remains on hold and its code is unmerged.

Cycle 2 assigns Claude IMP-P3-008 Password Recovery, the remaining IMP-P3-009 Username Availability endpoint, and IMP-P3-002 email-verification acceptance testing on `feat/claude-imp-p3-002-008-009-auth-completion`. Codex owns IMP-P3-012 Authentication Rate Limiting on `feat/codex-imp-p3-012-auth-rate-limiting`; Claude must consume Codex's shared interface. The assigned branch refs were not present in the verified GitHub branch listing, so branch publication and implementation are not claimed. P3-009 remains In testing until the endpoint and outstanding policy/acceptance evidence are resolved. P3-002 remains In testing until real staging delivery and confirmation are recorded. P3-003 remains Deferred — Post-MVP; P3-013 is Deferred — Owner Decision, with no MFA implementation assigned. No frontend, KYC-verification or financial implementation is assigned in this cycle.

PR #12 implemented and documented the approved profile-image extension under API §15 using private PostgreSQL storage, authenticated ownership, image normalization, transactional consistency and cleanup. It adds no task ID or duplicate frontend task. Preserve the separately recorded follow-ups for image retention/secure erasure, load/animated-image and upload-timeout evidence, and any future storage decision; these do not erase the merged acceptance.

## Phase 4 · KYC AND COMPLIANCE FOUNDATIONS (§17)

`IMP-P4-001` is Completed and engineering-accepted. PR [#5](https://github.com/Kenule-Africa/Daypay/pull/5) merged to `main` at `26d5023a02f0ed55fe3c95f1fb2b61394dd99c3f`; backend and frontend CI passed in run [45467027795](https://github.com/Kenule-Africa/Daypay/actions/runs/45467027795). `IMP-P4-002` is Completed — Engineering Accepted. Claude implemented it on the assigned branch; PR [#6](https://github.com/Kenule-Africa/Daypay/pull/6) merged at `d22181cc5bad7f46a87330ac25d8389ac1631d18` after backend and frontend checks passed on final implementation commit `96aa8e5deb1c95b3ff3500e406a81cdd54157576` in run [45487110380](https://github.com/Kenule-Africa/Daypay/actions/runs/45487110380). `IMP-P4-003` and `IMP-P4-004` are Completed — Engineering Accepted; Claude's PR [#7](https://github.com/Kenule-Africa/Daypay/pull/7) merged at `b1b9e2b4879f9b8603df29437ee67eb5ca44df23`, with backend/frontend CI passing on final commit `2f61d79b3206edb397fa6ad0d00d4b0076d43c97` in run [45499292388](https://github.com/Kenule-Africa/Daypay/actions/runs/45499292388). These are engineering acceptances, not production approvals; Phase 4 is not complete. See [PROGRESS.md](PROGRESS.md#2026-11-24--imp-p4-003-and-p4-004-engineering-acceptance), and the open dependencies in [DECISIONS.md](DECISIONS.md) and [BLOCKERS.md](BLOCKERS.md).

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P4-001 | KYC state machine. | Completed |
| IMP-P4-002 | KYC application service. | Completed |
| IMP-P4-003 | KYC provider interface. | Completed |
| IMP-P4-004 | Mock KYC adapter. | Completed |
| IMP-P4-005 | KYC API endpoints. | Blocked |
| IMP-P4-006 | Customer restriction engine. | Blocked |
| IMP-P4-007 | Transaction limit foundation. | Blocked |
| IMP-P4-008 | Compliance audit events. | Backlog |
| IMP-P4-009 | KYC review workflow. | Backlog |
| IMP-P4-010 | KYC provider integration after confirmation. | Backlog |

IMP-P4-005–007 are Blocked by the repository owner's explicit hold on further KYC verification implementation. Their tested code remains only in open, unmerged draft PR #10. IMP-P4-008–010 remain Backlog and unassigned. IMP-P4-001–004 retain their accepted statuses; Phase 4 is not complete. The hold does not cancel KYC requirements or authorize financial operations.

## Phase 5 · FINANCIAL LEDGER IMPLEMENTATION (§18)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P5-001 | Define exact-money types. | Completed |
| IMP-P5-002 | Define asset identifiers and precision rules. | Completed |
| IMP-P5-003 | Implement ledger accounts. | Completed |
| IMP-P5-004 | Implement journal headers. | Completed |
| IMP-P5-005 | Implement journal entries. | Completed |
| IMP-P5-006 | Implement balanced journal validation. | Completed |
| IMP-P5-007 | Implement posting service. | Completed |
| IMP-P5-008 | Implement customer available accounts. | Completed |
| IMP-P5-009 | Implement customer reserved accounts. | Completed |
| IMP-P5-010 | Implement reservations. | Completed |
| IMP-P5-011 | Implement reservation release. | Completed |
| IMP-P5-012 | Implement balance projections. | Completed |
| IMP-P5-013 | Implement accounting event idempotency. | Completed |
| IMP-P5-014 | Implement compensating journals. | Completed |
| IMP-P5-015 | Implement reconciliation foundations. | Completed |
| IMP-P5-016 | Implement concurrency controls. | Completed |
| IMP-P5-017 | Implement financial invariant tests. | Completed |

IMP-P5-001–017 are Completed — Engineering Accepted: PR #18 (Claude, `feat/claude-phase5-financial-ledger`, including its independent-review remediation in [PHASE5-LEDGER-SECURITY-REVIEW.md](PHASE5-LEDGER-SECURITY-REVIEW.md)) merged to `main` at `5ddb1c1`, and GitHub Actions run [45905299130](https://github.com/Kenule-Africa/Daypay/actions/runs/45905299130) passed on that merge commit. The ledger is persisted by migration `202611260001_financial_ledger.sql` with grants in `infrastructure/postgres/ledger-roles.psql` and `ledger-grants.psql`; the PostgreSQL suite is `backend/crates/deypay-infrastructure/tests/ledger.rs`. Engineering acceptance is not production activation: customer movements stay refused until capability activations, operator bindings and authenticated provider evidence exist, and the Finance, Legal, custody and provider gates in [BLOCKERS.md](BLOCKERS.md) remain open.

## Phase 6 · WALLET AND ASSET MANAGEMENT (§19)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P6-001 | Asset registry. | In review |
| IMP-P6-002 | Network registry. | In review |
| IMP-P6-003 | Token contract registry. | In review |
| IMP-P6-004 | Customer wallet profiles. | In review |
| IMP-P6-005 | Wallet status management. | In review |
| IMP-P6-006 | Wallet balance APIs. | In review |
| IMP-P6-007 | Deposit address domain model. | In review |
| IMP-P6-008 | Provider address provisioning interface. | In review |
| IMP-P6-009 | Wallet restrictions. | In review |
| IMP-P6-010 | Wallet tests. | In review |

IMP-P6-001–010 are implemented by Claude on `feat/claude-phase6-wallet-asset-management` and are In review pending owner review and merge of the pull request. Migration `202611261401_wallet_asset_management.sql` extends the Phase 2 wallet tables and the existing provider capability register; grants are in `infrastructure/postgres/wallet-roles.psql` and `wallet-grants.psql`; tests are `backend/crates/deypay-infrastructure/tests/wallets.rs`, `backend/crates/deypay-api/tests/cases/wallet_http.rs` and the frontend wallet tests. USDC and USDT wallet profiles, ledger-backed balances, status management, restrictions, and the wallet APIs and pages are functional. IMP-P6-007/008 deliver the deposit-address model, the durable provisioning state machine and the provider-independent `DepositAddressProvider` port only: no provider adapter exists; no network, token contract or provider capability is enabled; and external address provisioning remains Blocked under [BLOCKERS.md](BLOCKERS.md) until Nuvion (or another approved provider) capabilities are verified and approved. No deposit, transfer or withdrawal is enabled.

## Phase 7 · INTERNAL TRANSFERS (§20)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P7-001 | Recipient resolution. | Backlog |
| IMP-P7-002 | Username lookup controls. | Backlog |
| IMP-P7-003 | Transfer request validation. | Backlog |
| IMP-P7-004 | Customer eligibility checks. | Backlog |
| IMP-P7-005 | Balance validation. | Backlog |
| IMP-P7-006 | Transfer authorization. | Backlog |
| IMP-P7-007 | Atomic ledger posting. | Backlog |
| IMP-P7-008 | Idempotency. | Backlog |
| IMP-P7-009 | Transaction history integration. | Backlog |
| IMP-P7-010 | Transfer tests. | Backlog |

## Phase 8 · PROVIDER INTEGRATION FRAMEWORK (§21)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P8-001 | Define provider capability interfaces. | Backlog |
| IMP-P8-002 | Define canonical provider errors. | Backlog |
| IMP-P8-003 | Define provider operation states. | Backlog |
| IMP-P8-004 | Implement provider HTTP client foundation. | Backlog |
| IMP-P8-005 | Implement credential isolation. | Backlog |
| IMP-P8-006 | Implement request correlation. | Backlog |
| IMP-P8-007 | Implement provider operation persistence. | Backlog |
| IMP-P8-008 | Implement webhook receiver. | Backlog |
| IMP-P8-009 | Implement webhook verification. | Backlog |
| IMP-P8-010 | Implement webhook inbox. | Backlog |
| IMP-P8-011 | Implement durable worker processing. | Backlog |
| IMP-P8-012 | Implement provider status polling. | Backlog |
| IMP-P8-013 | Implement mock provider. | Backlog |
| IMP-P8-014 | Implement Nuvion adapter for confirmed capabilities. | Backlog |

## Phase 9 · STABLECOIN DEPOSITS (§22)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P9-001 | Deposit address provisioning. | Backlog |
| IMP-P9-002 | Deposit event ingestion. | Backlog |
| IMP-P9-003 | Deposit identification. | Backlog |
| IMP-P9-004 | Blockchain confirmation tracking. | Backlog |
| IMP-P9-005 | Compliance screening integration. | Backlog |
| IMP-P9-006 | Deposit ledger credit. | Backlog |
| IMP-P9-007 | Duplicate event prevention. | Backlog |
| IMP-P9-008 | Deposit status API. | Backlog |
| IMP-P9-009 | Deposit reconciliation. | Backlog |
| IMP-P9-010 | Deposit failure tests. | Backlog |

## Phase 10 · EXTERNAL STABLECOIN WITHDRAWALS (§23)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P10-001 | Withdrawal quote service. | Backlog |
| IMP-P10-002 | Address validation. | Backlog |
| IMP-P10-003 | Network validation. | Backlog |
| IMP-P10-004 | Fee calculation. | Backlog |
| IMP-P10-005 | Step-up authorization. | Backlog |
| IMP-P10-006 | Balance reservation. | Backlog |
| IMP-P10-007 | Provider execution. | Backlog |
| IMP-P10-008 | Transaction state tracking. | Backlog |
| IMP-P10-009 | Provider timeout handling. | Backlog |
| IMP-P10-010 | Settlement verification. | Backlog |
| IMP-P10-011 | Final ledger accounting. | Backlog |
| IMP-P10-012 | Withdrawal reconciliation. | Backlog |

## Phase 11 · STABLECOIN CONVERSION (§24)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P11-001 | FX quote domain model. | Backlog |
| IMP-P11-002 | Quote provider interface. | Backlog |
| IMP-P11-003 | Quote expiration. | Backlog |
| IMP-P11-004 | Fee and spread handling. | Backlog |
| IMP-P11-005 | Conversion authorization. | Backlog |
| IMP-P11-006 | Source asset reservation. | Backlog |
| IMP-P11-007 | Conversion execution. | Backlog |
| IMP-P11-008 | Destination asset accounting. | Backlog |
| IMP-P11-009 | Failure and recovery handling. | Backlog |
| IMP-P11-010 | Conversion reconciliation. | Backlog |

## Phase 12 · NGN ON-RAMP (§25)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P12-001 | Confirm provider capabilities. | Backlog |
| IMP-P12-002 | Define funding order model. | Backlog |
| IMP-P12-003 | Implement funding quote service. | Backlog |
| IMP-P12-004 | Implement funding instructions. | Backlog |
| IMP-P12-005 | Implement payment confirmation. | Backlog |
| IMP-P12-006 | Implement conversion or stablecoin delivery. | Backlog |
| IMP-P12-007 | Implement customer credit. | Backlog |
| IMP-P12-008 | Implement funding reconciliation. | Backlog |
| IMP-P12-009 | Implement exception handling. | Backlog |

## Phase 13 · NGN OFF-RAMP (§26)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P13-001 | Bank directory integration. | Backlog |
| IMP-P13-002 | Account verification. | Backlog |
| IMP-P13-003 | Beneficiary management. | Backlog |
| IMP-P13-004 | Off-ramp quotes. | Backlog |
| IMP-P13-005 | Stablecoin reservation. | Backlog |
| IMP-P13-006 | Conversion execution. | Backlog |
| IMP-P13-007 | NGN payout execution. | Backlog |
| IMP-P13-008 | Payout status tracking. | Backlog |
| IMP-P13-009 | Failed payout recovery. | Backlog |
| IMP-P13-010 | Reconciliation. | Backlog |

## Phase 14 · PAY ANYONE (§27)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P14-001 | Third-party beneficiary model. | Backlog |
| IMP-P14-002 | Bank account verification. | Backlog |
| IMP-P14-003 | Recipient confirmation. | Backlog |
| IMP-P14-004 | Pay Anyone quote service. | Backlog |
| IMP-P14-005 | Customer authorization. | Backlog |
| IMP-P14-006 | Stablecoin reservation. | Backlog |
| IMP-P14-007 | Conversion orchestration. | Backlog |
| IMP-P14-008 | Bank payout orchestration. | Backlog |
| IMP-P14-009 | Settlement tracking. | Backlog |
| IMP-P14-010 | Failed payout recovery. | Backlog |
| IMP-P14-011 | Refund or reconversion handling. | Backlog |
| IMP-P14-012 | Receipt generation. | Backlog |
| IMP-P14-013 | Reconciliation. | Backlog |
| IMP-P14-014 | End-to-end tests. | Backlog |

## Phase 15 · TRANSACTION HISTORY AND NOTIFICATIONS (§28)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P15-001 | Unified transaction query service. | Backlog |
| IMP-P15-002 | Transaction filtering. | Backlog |
| IMP-P15-003 | Cursor pagination. | Backlog |
| IMP-P15-004 | Transaction details. | Backlog |
| IMP-P15-005 | Receipt generation. | Backlog |
| IMP-P15-006 | Notification event model. | Backlog |
| IMP-P15-007 | Notification worker. | Backlog |
| IMP-P15-008 | Email notifications. | Backlog |
| IMP-P15-009 | In-app notifications. | Backlog |
| IMP-P15-010 | Notification preferences. | Backlog |

## Phase 16 · ADMINISTRATIVE AND RECONCILIATION TOOLS (§29)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P16-001 | Administrative authentication. | Backlog |
| IMP-P16-002 | Administrative RBAC. | Backlog |
| IMP-P16-003 | Customer management. | Backlog |
| IMP-P16-004 | KYC review interface. | Backlog |
| IMP-P16-005 | Transaction monitoring. | Backlog |
| IMP-P16-006 | Provider operation monitoring. | Backlog |
| IMP-P16-007 | Reconciliation runs. | Backlog |
| IMP-P16-008 | Reconciliation exception cases. | Backlog |
| IMP-P16-009 | Financial adjustment requests. | Backlog |
| IMP-P16-010 | Maker-checker approvals. | Backlog |
| IMP-P16-011 | Audit log viewer. | Backlog |
| IMP-P16-012 | Emergency financial operation suspension. | Backlog |

## Phase 17 · FRONTEND INTEGRATION AND USER EXPERIENCE (§30)

The roadmap assigns no task IDs here. Track its existing integration sections directly.

| Roadmap section | Work | Status |
|---|---|---|
| §30.1 | Authentication Screens | Backlog |
| §30.2 | Customer Onboarding | Backlog |
| §30.3 | Wallet Dashboard | Backlog |
| §30.4 | Send and Receive | Backlog |
| §30.5 | Conversion | Backlog |
| §30.6 | NGN Payments | Backlog |
| §30.7 | Transaction History | Backlog |
| §30.8 | Security | Backlog |
| §30.9 | Frontend Requirements | Backlog |

The early account frontend slice is a named milestone in roadmap §30, using existing P1-004 and §30.1, §30.2, relevant §30.8 and §30.9. It includes a dashboard shell and profile-management interfaces, adds no task ID or assignee, and does not complete Phase 17. It follows account-backend readiness and precedes financial implementation; apply security checks during the slice.

## Phase 18 · SECURITY HARDENING AND QUALITY ASSURANCE (§31)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P18-001 | Authentication security testing. | Backlog |
| IMP-P18-002 | Authorization testing. | Backlog |
| IMP-P18-003 | Financial invariant testing. | Backlog |
| IMP-P18-004 | Provider failure testing. | Backlog |
| IMP-P18-005 | Webhook security testing. | Backlog |
| IMP-P18-006 | Database concurrency testing. | Backlog |
| IMP-P18-007 | Load testing. | Backlog |
| IMP-P18-008 | Vulnerability scanning. | Backlog |
| IMP-P18-009 | Independent penetration testing. | Backlog |
| IMP-P18-010 | Backup restoration testing. | Backlog |
| IMP-P18-011 | Disaster recovery testing. | Backlog |
| IMP-P18-012 | Full regression testing. | Backlog |
| IMP-P18-013 | Financial reconciliation validation. | Backlog |

## Phase 19 · STAGING, PILOT AND PRODUCTION PREPARATION (§32)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P19-001 | Provision production infrastructure. | Backlog |
| IMP-P19-002 | Configure production secrets. | Backlog |
| IMP-P19-003 | Configure production database. | Backlog |
| IMP-P19-004 | Configure monitoring. | Backlog |
| IMP-P19-005 | Configure backups. | Backlog |
| IMP-P19-006 | Configure alerting. | Backlog |
| IMP-P19-007 | Validate provider credentials. | Backlog |
| IMP-P19-008 | Validate legal and regulatory requirements. | Backlog |
| IMP-P19-009 | Conduct production readiness review. | Backlog |
| IMP-P19-010 | Conduct controlled pilot. | Backlog |
| IMP-P19-011 | Reconcile pilot transactions. | Backlog |
| IMP-P19-012 | Approve phased rollout. | Backlog |
