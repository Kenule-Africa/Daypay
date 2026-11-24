# Roadmap task register

[Document 08](../08-IMPLEMENTATION-PLAN.md) owns task definitions, sequence, phase deliverables and exit criteria. This register tracks its 220 task IDs. The section number on each phase locates the source. Phase 17 uses section references.

For backlog items, the roadmap task wording and phase objective describe the work. At selection, record the responsible owner, priority, related specification, dependencies, task-specific acceptance and test requirements in the active work record below. Use the status values in §37.3. Completed entries refer to dated verification in [PROGRESS.md](PROGRESS.md). Open decisions and external dependencies are in [DECISIONS.md](DECISIONS.md) and [BLOCKERS.md](BLOCKERS.md).

The product owner authorized eligible Phase 2 foundation work in parallel on 2026-11-18. GitHub Actions commit `02fda26633b38946f1bb4d15d0ad86cee0a57656` completed the resolved lockfile, backend and frontend verification on 2026-11-18.

## Active work record

For each phase, apply its exit criteria in Document 08 before starting the next phase. Record dated evidence for runtime behavior, unit and integration tests, applicable security controls and failure recovery. For financial work also verify ledger invariants, idempotency, concurrency and recovery. Identify the tested revision and environment, along with any gaps. Compilation alone cannot close a phase.

| Task | Owner / priority | Dependencies and requirement | Acceptance and verification |
|---|---|---|---|
| IMP-P1-002 | Backend engineering / P0 | Repository baseline, roadmap §§9–10, 14 | Cargo resolves the current workspace with its reviewed lockfile. |
| IMP-P1-003 | Backend engineering / P0 | IMP-P1-002; architecture §39 | Cargo metadata lists all six planned crates with API and worker binary targets. Format, lint and build/test the workspace and record the outcome. |
| IMP-P1-004 | Frontend engineering / P0 | Repository baseline, roadmap §14 | Build and start the Next.js app, check lint and types, and exercise the page over HTTP. See [PROGRESS.md](PROGRESS.md). |
| IMP-P1-005 / 006 | Infrastructure / P0 | Local database settings and PostgreSQL bootstrap, roadmap §14 | Start PostgreSQL with Compose and run the role, privilege and connection checks as the runtime login. |
| IMP-P1-007 / 008 / 009 | Backend engineering / P0 | Current workspace and PostgreSQL, roadmap §14 | Exercise configuration validation, inspect JSON logs for secret leakage, and verify API liveness and readiness with the database available and unavailable. |
| IMP-P1-010 / 011 / 012 | Engineering / P0 | Backend, frontend and development database, roadmap §14 | Run the workflow on GitHub and record the lint, formatting, build, unit and integration results. |

## Parallel assignments

Only current unstarted implementation assignments are listed here. Completed assignments and their acceptance evidence are recorded in [PROGRESS.md](PROGRESS.md). These assignments do not change task statuses or acceptance criteria.

| Agent | Task | Branch | Owned modules/files | Dependencies | Status | PR / acceptance evidence |
|---|---|---|---|---|---|---|
| Codex | IMP-P3-004 Password Hashing | `feat/codex-imp-p3-004-password-hashing` | `deypay-application/src/passwords.rs`; `deypay-infrastructure/src/password_hashing.rs`; focused password/registration tests; existing password enrollment and configuration interfaces only | Phase 2 password credential schema; IMP-P3-001 registration; IMP-P3-009 where registration collision behavior is shared | Planned / Backlog | Not started |

Shared coordination boundaries:

- Neither agent may change migrations, shared API contracts, registration or KYC legal/security requirements, financial invariants, or the other agent's owned module.
- Codex must coordinate before touching registration, password credential persistence or shared test fixtures.
- Claude must coordinate before touching identity status, account restrictions, review/audit boundaries or shared test fixtures.
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

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P3-001 | Registration. | Completed |
| IMP-P3-002 | Email verification. | In testing |
| IMP-P3-003 | Phone verification. | Deferred — Post-MVP |
| IMP-P3-004 | Password hashing. | Backlog |
| IMP-P3-005 | Login. | Backlog |
| IMP-P3-006 | Session management. | Backlog |
| IMP-P3-007 | Logout. | Backlog |
| IMP-P3-008 | Password recovery. | Backlog |
| IMP-P3-009 | Username uniqueness. | In testing |
| IMP-P3-010 | Customer profile. | Backlog |
| IMP-P3-011 | Session revocation. | Backlog |
| IMP-P3-012 | Authentication rate limiting. | Backlog |
| IMP-P3-013 | MFA and step-up authentication foundations. | Backlog |

## Phase 4 · KYC AND COMPLIANCE FOUNDATIONS (§17)

`IMP-P4-001` is Completed and engineering-accepted. PR [#5](https://github.com/Kenule-Africa/Daypay/pull/5) merged to `main` at `26d5023a02f0ed55fe3c95f1fb2b61394dd99c3f`; backend and frontend CI passed in run [45467027795](https://github.com/Kenule-Africa/Daypay/actions/runs/45467027795). `IMP-P4-002` is Completed — Engineering Accepted. Claude implemented it on the assigned branch; PR [#6](https://github.com/Kenule-Africa/Daypay/pull/6) merged at `d22181cc5bad7f46a87330ac25d8389ac1631d18` after backend and frontend checks passed on final implementation commit `96aa8e5deb1c95b3ff3500e406a81cdd54157576` in run [45487110380](https://github.com/Kenule-Africa/Daypay/actions/runs/45487110380). `IMP-P4-003` and `IMP-P4-004` are Completed — Engineering Accepted; Claude's PR [#7](https://github.com/Kenule-Africa/Daypay/pull/7) merged at `b1b9e2b4879f9b8603df29437ee67eb5ca44df23`, with backend/frontend CI passing on final commit `2f61d79b3206edb397fa6ad0d00d4b0076d43c97` in run [45499292388](https://github.com/Kenule-Africa/Daypay/actions/runs/45499292388). These are engineering acceptances, not production approvals; Phase 4 is not complete. See [PROGRESS.md](PROGRESS.md#2026-11-24--imp-p4-003-and-p4-004-engineering-acceptance), and the open dependencies in [DECISIONS.md](DECISIONS.md) and [BLOCKERS.md](BLOCKERS.md).

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P4-001 | KYC state machine. | Completed |
| IMP-P4-002 | KYC application service. | Completed |
| IMP-P4-003 | KYC provider interface. | Completed |
| IMP-P4-004 | Mock KYC adapter. | Completed |
| IMP-P4-005 | KYC API endpoints. | Backlog |
| IMP-P4-006 | Customer restriction engine. | Backlog |
| IMP-P4-007 | Transaction limit foundation. | Backlog |
| IMP-P4-008 | Compliance audit events. | Backlog |
| IMP-P4-009 | KYC review workflow. | Backlog |
| IMP-P4-010 | KYC provider integration after confirmation. | Backlog |

## Phase 5 · FINANCIAL LEDGER IMPLEMENTATION (§18)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P5-001 | Define exact-money types. | Backlog |
| IMP-P5-002 | Define asset identifiers and precision rules. | Backlog |
| IMP-P5-003 | Implement ledger accounts. | Backlog |
| IMP-P5-004 | Implement journal headers. | Backlog |
| IMP-P5-005 | Implement journal entries. | Backlog |
| IMP-P5-006 | Implement balanced journal validation. | Backlog |
| IMP-P5-007 | Implement posting service. | Backlog |
| IMP-P5-008 | Implement customer available accounts. | Backlog |
| IMP-P5-009 | Implement customer reserved accounts. | Backlog |
| IMP-P5-010 | Implement reservations. | Backlog |
| IMP-P5-011 | Implement reservation release. | Backlog |
| IMP-P5-012 | Implement balance projections. | Backlog |
| IMP-P5-013 | Implement accounting event idempotency. | Backlog |
| IMP-P5-014 | Implement compensating journals. | Backlog |
| IMP-P5-015 | Implement reconciliation foundations. | Backlog |
| IMP-P5-016 | Implement concurrency controls. | Backlog |
| IMP-P5-017 | Implement financial invariant tests. | Backlog |

## Phase 6 · WALLET AND ASSET MANAGEMENT (§19)

| Task ID | Roadmap task | Status |
|---|---|---|
| IMP-P6-001 | Asset registry. | Backlog |
| IMP-P6-002 | Network registry. | Backlog |
| IMP-P6-003 | Token contract registry. | Backlog |
| IMP-P6-004 | Customer wallet profiles. | Backlog |
| IMP-P6-005 | Wallet status management. | Backlog |
| IMP-P6-006 | Wallet balance APIs. | Backlog |
| IMP-P6-007 | Deposit address domain model. | Backlog |
| IMP-P6-008 | Provider address provisioning interface. | Backlog |
| IMP-P6-009 | Wallet restrictions. | Backlog |
| IMP-P6-010 | Wallet tests. | Backlog |

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
