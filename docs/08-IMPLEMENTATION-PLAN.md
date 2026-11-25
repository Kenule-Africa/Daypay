# DEYPAY (DP)
# SOFTWARE IMPLEMENTATION PLAN AND ENGINEERING EXECUTION ROADMAP
## Development Strategy, Technical Work Breakdown, Milestones, Dependencies, Delivery Governance and Production Readiness

**Document ID:** DP-IMP-008  
**Version:** 1.0  
**Status:** Proposed Engineering Execution Baseline  
**Project:** DEYPAY  
**Organization:** Kenule Africa  
**Initial Market:** Nigeria  
**Document Type:** Software Implementation Plan  
**Filename:** `08-IMPLEMENTATION-PLAN.md`  
**Date:** 18 November 2026  
**Classification:** Confidential — Internal Engineering and Product Management

---

# DOCUMENT CONTROL

| Field | Description |
|---|---|
| Document Title | DEYPAY Software Implementation Plan and Engineering Execution Roadmap |
| Document ID | DP-IMP-008 |
| Version | 1.0 |
| Product | DEYPAY |
| Organization | Kenule Africa |
| Initial Market | Nigeria |
| Architecture | Rust Modular Monolith |
| Backend | Rust, Axum, Tokio, SQLx |
| Frontend | Next.js, React, TypeScript |
| Database | PostgreSQL |
| Infrastructure | Docker, Managed Hosting, CI/CD |
| Initial Customer Assets | USDC and USDT |
| Fiat Settlement Currency | NGN |
| Custody Model | Third-Party Custodial Infrastructure |
| Preferred Provider Candidate | Nuvion |
| Implementation Status | Not Started — Pending Baseline Approval |
| Approval Status | Pending |

## RELATED DOCUMENTS

This implementation plan must be read alongside the following documents:

1. `01-FRS.md` — Functional Requirements Specification.
2. `02-SYSTEM-ARCHITECTURE.md` — System Architecture Specification.
3. `03-DATABASE-DESIGN.md` — Database Design Specification.
4. `04-API-SPECIFICATION.md` — API Specification and Contract Design.
5. `05-LEDGER-ACCOUNTING.md` — Financial Ledger and Accounting Design.
6. `06-SECURITY-COMPLIANCE.md` — Security Architecture and Threat Model.
7. `07-TEST-STRATEGY.md` — Software Test Strategy and Quality Assurance.

The eight documents collectively establish the initial engineering baseline for DEYPAY.

They do not replace the need for implementation-level schemas, source code, infrastructure configuration, test evidence, provider contracts or applicable regulatory approvals.

---

# TABLE OF CONTENTS

1. Executive Summary
2. Implementation Objectives
3. Implementation Scope
4. Confirmed Architectural Direction
5. Engineering Execution Principles
6. Project Governance and Decision Ownership
7. Prerequisites and Readiness Assessment
8. Workstreams and Dependencies
9. Technology Stack and Version Policy
10. Repository and Project Structure
11. Development Environment Setup
12. Engineering Standards and Development Workflow
13. Phase 0 — Requirements and Architecture Baseline
14. Phase 1 — Project Foundation and Infrastructure
15. Phase 2 — Database and Domain Foundations
16. Phase 3 — Authentication and Customer Identity
17. Phase 4 — KYC and Compliance Foundations
18. Phase 5 — Financial Ledger Implementation
19. Phase 6 — Wallet and Asset Management
20. Phase 7 — Internal Transfers
21. Phase 8 — Provider Integration Framework
22. Phase 9 — Stablecoin Deposits
23. Phase 10 — External Stablecoin Withdrawals
24. Phase 11 — Stablecoin Conversion
25. Phase 12 — NGN On-Ramp
26. Phase 13 — NGN Off-Ramp
27. Phase 14 — Pay Anyone
28. Phase 15 — Transaction History and Notifications
29. Phase 16 — Administrative and Reconciliation Tools
30. Phase 17 — Frontend Integration and User Experience
31. Phase 18 — Security Hardening and Quality Assurance
32. Phase 19 — Staging, Pilot and Production Preparation
33. Implementation Dependency Graph
34. Sprint Planning and Delivery Cadence
35. Indicative Timeline and Milestones
36. Minimum Engineering Team
37. Work Breakdown and Task Management
38. Definition of Ready
39. Definition of Done
40. Database Migration and Data Governance
41. API Contract Implementation
42. Provider Integration and Nuvion Validation
43. Financial Integrity and Reconciliation Controls
44. Security and Compliance Implementation Gates
45. Testing and Quality Gates
46. CI/CD and Deployment Strategy
47. Observability and Operational Readiness
48. Release Management and Feature Flags
49. Budget and Infrastructure Cost Management
50. Risk Register and Mitigation Plan
51. Dependency and Decision Register
52. Acceptance Criteria by Milestone
53. MVP Launch Readiness Checklist
54. Post-Launch Stabilization and Scaling
55. Immediate Engineering Action Plan
56. Final Implementation Strategy and Approval

---

# 1. EXECUTIVE SUMMARY

DEYPAY is a stablecoin-first financial application designed initially for the Nigerian market.

Its initial product scope includes:

- Customer registration and authentication.
- Identity verification and compliance controls.
- USDC and USDT customer wallets.
- Blockchain deposits.
- Internal stablecoin transfers.
- External stablecoin withdrawals.
- USDC-to-USDT and USDT-to-USDC conversions.
- NGN on-ramp.
- NGN off-ramp.
- Pay Anyone payments to supported Nigerian bank accounts.
- Transaction history.
- Notifications.
- Administrative oversight.
- Financial reconciliation.

The implementation will follow a modular monolith architecture.

The backend will be developed in Rust using Axum, Tokio and SQLx.

PostgreSQL will serve as the authoritative persistence layer for customer records, transactions, financial journals and durable operational workflows.

The frontend will use Next.js, React and TypeScript, initially delivered as a responsive Progressive Web Application.

External financial infrastructure will be accessed through provider adapters.

Nuvion is the preferred infrastructure candidate, subject to written capability confirmation, technical validation, commercial agreement and legal approval.

DEYPAY will maintain ownership of:

1. Customer identity.
2. Customer-facing APIs.
3. Business rules.
4. Authorization.
5. Financial ledger.
6. Transaction orchestration.
7. Transaction history.
8. Compliance workflow coordination.
9. Provider operation records.
10. Reconciliation.
11. Administrative controls.

The implementation strategy is to establish reliable financial foundations before enabling external movement of customer funds.

---

# 2. IMPLEMENTATION OBJECTIVES

## IMP-OBJ-001 — Deliver the MVP

Build the approved DEYPAY MVP according to the Functional Requirements Specification.

## IMP-OBJ-002 — Preserve Architectural Control

Ensure DEYPAY retains ownership of its application and financial business logic.

## IMP-OBJ-003 — Establish Financial Correctness

Implement an independent double-entry ledger with exact monetary accounting.

## IMP-OBJ-004 — Build Secure Financial Workflows

Require authentication, authorization, eligibility checks, balance verification and idempotency.

## IMP-OBJ-005 — Enable Provider Independence

Implement provider abstractions so infrastructure providers can be introduced or replaced without rewriting the entire application.

## IMP-OBJ-006 — Minimize Premature Complexity

Avoid unnecessary microservices, Kubernetes and distributed infrastructure during early development.

## IMP-OBJ-007 — Support Future Mobile Applications

Design backend APIs independently from the PWA.

## IMP-OBJ-008 — Establish Quality Gates

Ensure every critical feature is tested before release.

## IMP-OBJ-009 — Support Nigerian Market Requirements

Design NGN payment capabilities around approved legal and provider arrangements.

## IMP-OBJ-010 — Prepare for Controlled Production Launch

Establish security, monitoring, reconciliation, incident response and operational procedures.

---

# 3. IMPLEMENTATION SCOPE

## 3.1 MVP Functional Scope

| Capability | MVP Scope |
|---|---|
| Customer registration | Included |
| Email verification | Required for MVP contact verification |
| Phone number collection | Required at registration |
| Phone OTP and verification | Deferred — Post-MVP |
| Authentication and sessions | Included |
| Unique username | Included |
| KYC | Included |
| USDC wallet | Included |
| USDT wallet | Included |
| Stablecoin deposits | Included |
| Internal transfers | Included |
| External withdrawals | Included |
| USDC/USDT conversion | Included |
| NGN on-ramp | Included, provider-dependent |
| NGN off-ramp | Included, provider-dependent |
| Pay Anyone | Included, provider- and regulatory-dependent |
| Transaction history | Included |
| Notifications | Included |
| Beneficiaries and QR payments | Included |
| Payment links | Included, subject to approved risk controls |
| Support and dispute workflows | Included |
| Administrative dashboard | Included |
| Reconciliation | Included |
| Responsive PWA | Included |
| Native mobile application | Future phase |

## 3.2 Excluded Features

The initial implementation excludes:

- Consumer payment cards.
- Gift cards.
- Stock trading.
- Prediction markets.
- Lending.
- Unapproved digital assets.
- Unapproved blockchain networks.
- A general-purpose customer NGN stored-value wallet.

## 3.3 NGN Operating Model

The MVP will follow the previously selected Model B approach.

NGN will be used for:

- Funding.
- Conversion.
- Bank settlement.
- Treasury.
- Bank payouts.

DEYPAY will not expose a general-purpose customer NGN wallet balance in the initial MVP.

---

# 4. CONFIRMED ARCHITECTURAL DIRECTION

The following decisions form the implementation baseline.

## 4.1 Backend

Rust modular monolith.

## 4.2 HTTP Framework

Axum.

## 4.3 Async Runtime

Tokio.

## 4.4 Database

PostgreSQL.

## 4.5 Database Access

SQLx.

## 4.6 Frontend

Next.js, React and TypeScript.

## 4.7 API

REST over HTTPS with JSON.

## 4.8 API Documentation

OpenAPI 3.1 or an approved compatible version.

## 4.9 Financial Ledger

Independent immutable double-entry accounting.

## 4.10 Background Processing

PostgreSQL-backed durable jobs and transactional outbox.

## 4.11 Provider Architecture

Internal adapters with canonical DEYPAY domain models.

## 4.12 Custody

Third-party custodial infrastructure, subject to legal and technical validation.

## 4.13 Deployment

Containerized applications using Docker and managed infrastructure.

## 4.14 Future Architecture

Microservices may be considered only when supported by measurable operational requirements.

---

# 5. ENGINEERING EXECUTION PRINCIPLES

## 5.1 Build Foundations First

Authentication, database integrity, ledger accounting and authorization must precede production financial execution.

## 5.2 Implement Vertical Slices

Each feature should include:

- Domain logic.
- Application service.
- Persistence.
- API.
- Tests.
- Frontend integration where applicable.

## 5.3 Separate Financial Logic From Providers

Provider responses must not define DEYPAY's internal accounting rules.

## 5.4 Automate Verification

Critical tests must run in CI.

## 5.5 Maintain Traceability

Implementation tasks must reference the relevant requirements and specifications.

## 5.6 Prefer Reversible Decisions

Avoid irreversible infrastructure choices where simpler alternatives remain adequate.

## 5.7 Secure by Default

Sensitive features must begin with restrictive permissions.

## 5.8 Use Explicit Acceptance Criteria

Tasks must have measurable completion conditions.

## 5.9 Treat External Outcomes Carefully

Provider timeouts must not trigger unsafe retries.

## 5.10 Do Not Equate Code Completion With Launch Approval

Technical completion and production authorization are separate milestones.

---

# 6. PROJECT GOVERNANCE AND DECISION OWNERSHIP

## 6.1 Product Owner

Owns:

- Product priorities.
- Business requirements.
- MVP scope.
- User experience acceptance.
- Commercial decisions.

## 6.2 Engineering Lead

Owns:

- Technical architecture.
- Engineering standards.
- Implementation sequencing.
- Code quality.
- Technical delivery.

## 6.3 Backend Engineering

Owns:

- Rust services.
- Financial workflows.
- API implementation.
- Database integration.
- Provider adapters.

## 6.4 Frontend Engineering

Owns:

- PWA implementation.
- Customer journeys.
- Frontend API integration.
- Responsive design.
- Accessibility.

## 6.5 Security

Owns:

- Security requirements.
- Security review.
- Vulnerability management.
- Security acceptance.

## 6.6 Financial Operations

Owns:

- Financial control review.
- Reconciliation procedures.
- Treasury operations.
- Financial exception handling.

## 6.7 Compliance and Legal

Own:

- Regulatory interpretation.
- KYC/AML obligations.
- Provider contractual review.
- Launch authorization assessment.

## 6.8 Decision Register

Architecture and financial operating decisions must be recorded with:

- Decision ID.
- Description.
- Options.
- Selected approach.
- Rationale.
- Owner.
- Approval date.
- Revisit trigger.

---

# 7. PREREQUISITES AND READINESS ASSESSMENT

Before full implementation, verify the following.

## 7.1 Repository Access

The engineering team must have controlled access to the source repository.

## 7.2 Documentation Baseline

The eight core documents must be reviewed for consistency.

## 7.3 Development Tooling

Required tools must be installed and working.

## 7.4 Database Environment

A local PostgreSQL instance must be available.

## 7.5 Provider Documentation

Current provider API documentation must be accessible.

## 7.6 Provider Capability Register

Supported operations must be recorded as confirmed, unconfirmed or unsupported.

## 7.7 Security Ownership

A responsible security reviewer must be identified.

## 7.8 Financial Design Review

The chart of accounts, posting rules and reconciliation model must be approved before implementation of production financial features.

## 7.9 Legal Review

The intended Nigerian operating model must undergo legal assessment.

## 7.10 Readiness Outcome

Prerequisites must be classified as:

- Ready.
- Ready with constraints.
- Blocked.
- Not applicable.

---

# 8. WORKSTREAMS AND DEPENDENCIES

The project will be organized into eight principal workstreams.

| Workstream | Description |
|---|---|
| WS-01 | Engineering foundation |
| WS-02 | Identity and compliance |
| WS-03 | Financial ledger |
| WS-04 | Wallets and transfers |
| WS-05 | Provider integrations |
| WS-06 | NGN payments |
| WS-07 | Frontend and administration |
| WS-08 | Quality, security and release |

## 8.1 Parallel Work

Frontend design, API contract definition and provider capability research can proceed alongside backend foundations.

## 8.2 Critical Dependencies

External financial execution depends on:

- Customer eligibility.
- Authorization.
- Ledger reservations.
- Provider operation tracking.
- Idempotency.
- Reconciliation.

## 8.3 Blocked Work

A feature may be implemented against a simulator while production execution remains disabled pending external approvals.

---

# 9. TECHNOLOGY STACK AND VERSION POLICY

## 9.1 Backend Dependencies

Recommended categories:

- Rust stable toolchain.
- Axum.
- Tokio.
- SQLx.
- Serde.
- Reqwest.
- Tracing.
- UUID.
- Time.
- Approved exact-money types.
- OpenAPI tooling.

## 9.2 Frontend Dependencies

Recommended categories:

- Next.js.
- React.
- TypeScript.
- Form validation library.
- API client generation.
- Component testing framework.
- End-to-end testing framework.

## 9.3 Database

Use a supported PostgreSQL version.

PostgreSQL 16 or newer is an acceptable initial candidate, subject to hosting support and compatibility testing.

## 9.4 Version Pinning

Exact dependency versions must be recorded in lockfiles.

## 9.5 Upgrade Policy

Dependency upgrades require automated tests and security review.

---

# 10. REPOSITORY AND PROJECT STRUCTURE

Recommended monorepo:

```text
deypay/
├── README.md
├── .gitignore
├── .editorconfig
├── .env.example
├── .github/
│   └── workflows/
├── docs/
│   ├── 01-FRS.md
│   ├── 02-SYSTEM-ARCHITECTURE.md
│   ├── 03-DATABASE-DESIGN.md
│   ├── 04-API-SPECIFICATION.md
│   ├── 05-LEDGER-ACCOUNTING.md
│   ├── 06-SECURITY-COMPLIANCE.md
│   ├── 07-TEST-STRATEGY.md
│   └── 08-IMPLEMENTATION-PLAN.md
├── backend/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── crates/
│   │   ├── deypay-domain/
│   │   ├── deypay-application/
│   │   ├── deypay-infrastructure/
│   │   ├── deypay-api/
│   │   ├── deypay-worker/
│   │   └── deypay-common/
│   ├── migrations/
│   └── tests/
├── frontend/
│   ├── app/
│   ├── components/
│   ├── features/
│   ├── lib/
│   ├── public/
│   └── tests/
├── infrastructure/
│   ├── docker/
│   ├── compose/
│   ├── deployment/
│   └── monitoring/
├── scripts/
└── .github/
```

The duplicated `.github/` entry in this illustrative tree represents one repository-level directory; the actual repository must contain only one.

## 10.1 Domain Crate

Contains:

- Entities.
- Value objects.
- Financial rules.
- Domain errors.
- Domain interfaces.

## 10.2 Application Crate

Contains:

- Use cases.
- Transaction orchestration.
- Authorization coordination.
- Domain service coordination.

## 10.3 Infrastructure Crate

Contains:

- PostgreSQL repositories.
- Provider adapters.
- External HTTP clients.
- Storage integrations.

## 10.4 API Crate

Contains:

- Axum routes.
- Request DTOs.
- Response DTOs.
- Authentication middleware.
- HTTP error mapping.

## 10.5 Worker Crate

Contains:

- Durable job execution.
- Provider status polling.
- Webhook processing.
- Reconciliation jobs.

## 10.6 Common Crate

Contains carefully selected shared utilities.

It must not become a dumping ground for unrelated business logic.

---

# 11. DEVELOPMENT ENVIRONMENT SETUP

## 11.1 Required Software

Developers should install:

- Git.
- Rust toolchain.
- Cargo.
- Node.js supported LTS.
- Frontend package manager.
- Docker.
- PostgreSQL client.
- Code editor.
- SQLx tooling where needed.

## 11.2 Environment Variables

Provide `.env.example`.

Required configuration categories:

- Database connection.
- Application environment.
- API binding.
- Authentication configuration.
- Provider configuration.
- Logging configuration.
- Frontend API base URL.

## 11.3 Secrets

Real credentials must never be committed to the repository.

## 11.4 Local Services

Local development should support:

- PostgreSQL.
- API.
- Worker.
- Frontend.
- Mock provider.

## 11.5 Developer Onboarding

A new developer should be able to:

1. Clone the repository.
2. Install dependencies.
3. Start local infrastructure.
4. Apply migrations.
5. Start backend services.
6. Start frontend.
7. Run automated tests.

---

# 12. ENGINEERING STANDARDS AND DEVELOPMENT WORKFLOW

## 12.1 Branching

Use a documented branching policy.

Recommended:

- Protected main branch.
- Short-lived feature branches.
- Pull requests.
- Required review.

## 12.2 Code Review

All production changes require review.

Critical financial changes require review by a developer familiar with ledger and transaction integrity.

## 12.3 Commit Standards

Commits should clearly describe their changes.

## 12.4 Rust Quality

Required checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 12.5 Frontend Quality

Required checks:

- Linting.
- Type checking.
- Component tests.
- Production build validation.

## 12.6 Documentation

Implementation changes affecting approved contracts must update the relevant specification.

---

# 13. PHASE 0 — REQUIREMENTS AND ARCHITECTURE BASELINE

## Objective

Ensure the engineering team is building from a consistent set of requirements.

## Tasks

**IMP-P0-001:** Review all eight core documents.

**IMP-P0-002:** Resolve inconsistent document numbering and cross-references.

**IMP-P0-003:** Confirm MVP scope.

**IMP-P0-004:** Confirm the Model B NGN approach.

**IMP-P0-005:** Review the custody model.

**IMP-P0-006:** Confirm financial ledger posting rules.

**IMP-P0-007:** Review API contracts.

**IMP-P0-008:** Create the provider capability register.

**IMP-P0-009:** Identify legal and regulatory dependencies.

**IMP-P0-010:** Approve implementation priorities.

## Deliverables

- Approved requirements baseline.
- Resolved architecture decisions.
- Initial backlog.
- Dependency register.
- Provider capability register.

## Exit Criteria

No unresolved contradiction prevents foundational engineering work.

---

# 14. PHASE 1 — PROJECT FOUNDATION AND INFRASTRUCTURE

## Objective

Create a working development foundation.

## Tasks

**IMP-P1-001:** Initialize repository.

**IMP-P1-002:** Create Rust workspace.

**IMP-P1-003:** Create backend crates.

**IMP-P1-004:** Initialize Next.js frontend.

**IMP-P1-005:** Configure PostgreSQL.

**IMP-P1-006:** Configure Docker development services.

**IMP-P1-007:** Implement application configuration.

**IMP-P1-008:** Implement structured logging.

**IMP-P1-009:** Create health endpoints.

**IMP-P1-010:** Establish CI pipeline.

**IMP-P1-011:** Configure linting and formatting.

**IMP-P1-012:** Create initial integration test infrastructure.

## Deliverables

- Running Rust API.
- Running frontend.
- Running PostgreSQL.
- Working Docker development environment.
- Initial CI checks.

## Exit Criteria

A developer can run the full local application and execute tests successfully.

---

# 15. PHASE 2 — DATABASE AND DOMAIN FOUNDATIONS

## Objective

Establish reliable persistence and domain modeling.

## Tasks

**IMP-P2-001:** Create database migration framework.

**IMP-P2-002:** Implement customer identity tables.

**IMP-P2-003:** Implement authentication tables.

**IMP-P2-004:** Implement KYC tables.

**IMP-P2-005:** Implement asset registry.

**IMP-P2-006:** Implement wallet profiles.

**IMP-P2-007:** Implement transaction records.

**IMP-P2-008:** Implement provider operation records.

**IMP-P2-009:** Implement webhook inbox.

**IMP-P2-010:** Implement outbox and durable jobs.

**IMP-P2-011:** Implement audit tables.

**IMP-P2-012:** Implement repository interfaces.

## Deliverables

- Versioned migrations.
- Domain entities.
- Repository interfaces.
- Database integration tests.

## Exit Criteria

Core persistence structures are implemented and validated against PostgreSQL.

---

# 16. PHASE 3 — AUTHENTICATION AND CUSTOMER IDENTITY

## Objective

Implement secure customer identity and access.

## Tasks

**IMP-P3-001:** Registration.

**IMP-P3-002:** Email verification.

**IMP-P3-003:** Phone verification.

Deferred — Post-MVP by product owner decision. Phase 3 registration still requires an E.164 phone number. Email confirmation satisfies the contact requirement. Continue the MVP sequence with IMP-P3-004.

**IMP-P3-004:** Password hashing.

**IMP-P3-005:** Login.

**IMP-P3-006:** Session management.

**IMP-P3-007:** Logout.

**IMP-P3-008:** Password recovery.

**IMP-P3-009:** Username uniqueness.

**IMP-P3-010:** Customer profile.

**IMP-P3-011:** Session revocation.

**IMP-P3-012:** Authentication rate limiting.

**IMP-P3-013:** MFA and step-up authentication foundations.

## Deliverables

- Working authentication APIs.
- Customer profile APIs.
- Session management.
- Authentication tests.

## Exit Criteria

Customers can register and authenticate securely, and unauthorized requests are rejected.

MVP acceptance requires email confirmation while the collected phone remains unverified. Phone OTP is outside the MVP exit criteria. KYC and financial eligibility retain their own gates.

---

# 17. PHASE 4 — KYC AND COMPLIANCE FOUNDATIONS

## Objective

Implement customer eligibility and compliance workflow controls.

## Tasks

**IMP-P4-001:** KYC state machine.

**IMP-P4-002:** KYC application service.

**IMP-P4-003:** KYC provider interface.

**IMP-P4-004:** Mock KYC adapter.

**IMP-P4-005:** KYC API endpoints.

**IMP-P4-006:** Customer restriction engine.

**IMP-P4-007:** Transaction limit foundation.

**IMP-P4-008:** Compliance audit events.

**IMP-P4-009:** KYC review workflow.

**IMP-P4-010:** KYC provider integration after confirmation.

## Deliverables

- KYC workflow.
- Eligibility checks.
- Restriction enforcement.
- Mock provider tests.

## Exit Criteria

Financial actions are blocked when required customer eligibility conditions are not satisfied.

---

# 18. PHASE 5 — FINANCIAL LEDGER IMPLEMENTATION

## Objective

Build the authoritative financial accounting engine.

This phase is a critical prerequisite for customer financial operations.

## Tasks

**IMP-P5-001:** Define exact-money types.

**IMP-P5-002:** Define asset identifiers and precision rules.

**IMP-P5-003:** Implement ledger accounts.

**IMP-P5-004:** Implement journal headers.

**IMP-P5-005:** Implement journal entries.

**IMP-P5-006:** Implement balanced journal validation.

**IMP-P5-007:** Implement posting service.

**IMP-P5-008:** Implement customer available accounts.

**IMP-P5-009:** Implement customer reserved accounts.

**IMP-P5-010:** Implement reservations.

**IMP-P5-011:** Implement reservation release.

**IMP-P5-012:** Implement balance projections.

**IMP-P5-013:** Implement accounting event idempotency.

**IMP-P5-014:** Implement compensating journals.

**IMP-P5-015:** Implement reconciliation foundations.

**IMP-P5-016:** Implement concurrency controls.

**IMP-P5-017:** Implement financial invariant tests.

## Mandatory Invariants

1. Posted journals balance by asset.
2. Posted entries are immutable.
3. Financial events cannot post twice.
4. Customer available balances cannot be overspent.
5. Reservations cannot be consumed twice.
6. Balance projections can be reconstructed.
7. Reversals preserve historical accounting records.

## Deliverables

- Ledger posting engine.
- Reservation engine.
- Balance projection service.
- Financial test suite.

## Exit Criteria

All critical ledger invariants pass automated tests.

No external financial execution may be enabled before this phase is accepted.

---

# 19. PHASE 6 — WALLET AND ASSET MANAGEMENT

## Objective

Implement customer wallet profiles and supported asset management.

## Tasks

**IMP-P6-001:** Asset registry.

**IMP-P6-002:** Network registry.

**IMP-P6-003:** Token contract registry.

**IMP-P6-004:** Customer wallet profiles.

**IMP-P6-005:** Wallet status management.

**IMP-P6-006:** Wallet balance APIs.

**IMP-P6-007:** Deposit address domain model.

**IMP-P6-008:** Provider address provisioning interface.

**IMP-P6-009:** Wallet restrictions.

**IMP-P6-010:** Wallet tests.

## Deliverables

- USDC wallet profiles.
- USDT wallet profiles.
- Balance APIs.
- Network capability validation.

## Exit Criteria

Wallets correctly reflect DEYPAY ledger balances and approved capabilities.

---

# 20. PHASE 7 — INTERNAL TRANSFERS

## Objective

Implement off-chain transfers between DEYPAY customers.

## Tasks

**IMP-P7-001:** Recipient resolution.

**IMP-P7-002:** Username lookup controls.

**IMP-P7-003:** Transfer request validation.

**IMP-P7-004:** Customer eligibility checks.

**IMP-P7-005:** Balance validation.

**IMP-P7-006:** Transfer authorization.

**IMP-P7-007:** Atomic ledger posting.

**IMP-P7-008:** Idempotency.

**IMP-P7-009:** Transaction history integration.

**IMP-P7-010:** Transfer tests.

## Deliverables

- Internal transfer API.
- Recipient resolution.
- Transfer accounting.
- Transfer history.

## Exit Criteria

Internal transfers remain atomic and cannot overspend available balances.

---

# 21. PHASE 8 — PROVIDER INTEGRATION FRAMEWORK

## Objective

Create reusable infrastructure adapters.

## Tasks

**IMP-P8-001:** Define provider capability interfaces.

**IMP-P8-002:** Define canonical provider errors.

**IMP-P8-003:** Define provider operation states.

**IMP-P8-004:** Implement provider HTTP client foundation.

**IMP-P8-005:** Implement credential isolation.

**IMP-P8-006:** Implement request correlation.

**IMP-P8-007:** Implement provider operation persistence.

**IMP-P8-008:** Implement webhook receiver.

**IMP-P8-009:** Implement webhook verification.

**IMP-P8-010:** Implement webhook inbox.

**IMP-P8-011:** Implement durable worker processing.

**IMP-P8-012:** Implement provider status polling.

**IMP-P8-013:** Implement mock provider.

**IMP-P8-014:** Implement Nuvion adapter for confirmed capabilities.

## Deliverables

- Provider abstraction layer.
- Mock provider.
- Webhook infrastructure.
- Durable operation tracking.
- Initial Nuvion adapter.

## Exit Criteria

Provider operations can be executed and reconciled through controlled backend workflows.

---

# 22. PHASE 9 — STABLECOIN DEPOSITS

## Objective

Enable verified blockchain deposits.

## Tasks

**IMP-P9-001:** Deposit address provisioning.

**IMP-P9-002:** Deposit event ingestion.

**IMP-P9-003:** Deposit identification.

**IMP-P9-004:** Blockchain confirmation tracking.

**IMP-P9-005:** Compliance screening integration.

**IMP-P9-006:** Deposit ledger credit.

**IMP-P9-007:** Duplicate event prevention.

**IMP-P9-008:** Deposit status API.

**IMP-P9-009:** Deposit reconciliation.

**IMP-P9-010:** Deposit failure tests.

## Deliverables

- Deposit workflow.
- Deposit address API.
- Verified ledger credit.
- Deposit monitoring.

## Exit Criteria

Only verified eligible deposits create customer spendable balances.

---

# 23. PHASE 10 — EXTERNAL STABLECOIN WITHDRAWALS

## Objective

Enable secure external blockchain withdrawals.

## Tasks

**IMP-P10-001:** Withdrawal quote service.

**IMP-P10-002:** Address validation.

**IMP-P10-003:** Network validation.

**IMP-P10-004:** Fee calculation.

**IMP-P10-005:** Step-up authorization.

**IMP-P10-006:** Balance reservation.

**IMP-P10-007:** Provider execution.

**IMP-P10-008:** Transaction state tracking.

**IMP-P10-009:** Provider timeout handling.

**IMP-P10-010:** Settlement verification.

**IMP-P10-011:** Final ledger accounting.

**IMP-P10-012:** Withdrawal reconciliation.

## Deliverables

- Withdrawal quote API.
- Withdrawal execution API.
- Provider execution workflow.
- Reconciliation.

## Exit Criteria

Withdrawals cannot execute without authorization and cannot be duplicated through retries.

---

# 24. PHASE 11 — STABLECOIN CONVERSION

## Objective

Implement USDC/USDT conversion.

## Tasks

**IMP-P11-001:** FX quote domain model.

**IMP-P11-002:** Quote provider interface.

**IMP-P11-003:** Quote expiration.

**IMP-P11-004:** Fee and spread handling.

**IMP-P11-005:** Conversion authorization.

**IMP-P11-006:** Source asset reservation.

**IMP-P11-007:** Conversion execution.

**IMP-P11-008:** Destination asset accounting.

**IMP-P11-009:** Failure and recovery handling.

**IMP-P11-010:** Conversion reconciliation.

## Deliverables

- FX quote API.
- Conversion API.
- Multi-asset accounting.
- Conversion tests.

## Exit Criteria

Conversions preserve accurate customer entitlements and balanced per-asset accounting.

---

# 25. PHASE 12 — NGN ON-RAMP

## Objective

Enable customers to fund stablecoin wallets through approved NGN collection infrastructure.

## Tasks

**IMP-P12-001:** Confirm provider capabilities.

**IMP-P12-002:** Define funding order model.

**IMP-P12-003:** Implement funding quote service.

**IMP-P12-004:** Implement funding instructions.

**IMP-P12-005:** Implement payment confirmation.

**IMP-P12-006:** Implement conversion or stablecoin delivery.

**IMP-P12-007:** Implement customer credit.

**IMP-P12-008:** Implement funding reconciliation.

**IMP-P12-009:** Implement exception handling.

## Deliverables

- Funding quote API.
- Funding order API.
- Confirmed funding workflow.
- Reconciliation.

## Exit Criteria

Confirmed NGN funding results in correct stablecoin credit without creating an unauthorized NGN stored-value wallet.

---

# 26. PHASE 13 — NGN OFF-RAMP

## Objective

Enable approved stablecoin-funded NGN payouts to a customer's eligible bank account.

## Tasks

**IMP-P13-001:** Bank directory integration.

**IMP-P13-002:** Account verification.

**IMP-P13-003:** Beneficiary management.

**IMP-P13-004:** Off-ramp quotes.

**IMP-P13-005:** Stablecoin reservation.

**IMP-P13-006:** Conversion execution.

**IMP-P13-007:** NGN payout execution.

**IMP-P13-008:** Payout status tracking.

**IMP-P13-009:** Failed payout recovery.

**IMP-P13-010:** Reconciliation.

## Deliverables

- Bank account verification.
- Beneficiary management.
- Off-ramp API.
- Payout reconciliation.

## Exit Criteria

NGN payouts are correctly authorized, executed and reconciled.

---

# 27. PHASE 14 — PAY ANYONE

## Objective

Enable customers to send NGN to supported third-party Nigerian bank accounts using USDC or USDT.

## Tasks

**IMP-P14-001:** Third-party beneficiary model.

**IMP-P14-002:** Bank account verification.

**IMP-P14-003:** Recipient confirmation.

**IMP-P14-004:** Pay Anyone quote service.

**IMP-P14-005:** Customer authorization.

**IMP-P14-006:** Stablecoin reservation.

**IMP-P14-007:** Conversion orchestration.

**IMP-P14-008:** Bank payout orchestration.

**IMP-P14-009:** Settlement tracking.

**IMP-P14-010:** Failed payout recovery.

**IMP-P14-011:** Refund or reconversion handling.

**IMP-P14-012:** Receipt generation.

**IMP-P14-013:** Reconciliation.

**IMP-P14-014:** End-to-end tests.

## Deliverables

- Pay Anyone quote API.
- Bank payment API.
- Payment status API.
- Payment receipts.
- Financial recovery workflow.

## Exit Criteria

A customer can initiate an approved stablecoin-funded NGN payment to a third-party bank account, with correct authorization, accounting and settlement verification.

Production enablement requires separate legal and provider approval.

---

# 28. PHASE 15 — TRANSACTION HISTORY AND NOTIFICATIONS

## Objective

Provide customers with accurate financial activity records.

## Tasks

**IMP-P15-001:** Unified transaction query service.

**IMP-P15-002:** Transaction filtering.

**IMP-P15-003:** Cursor pagination.

**IMP-P15-004:** Transaction details.

**IMP-P15-005:** Receipt generation.

**IMP-P15-006:** Notification event model.

**IMP-P15-007:** Notification worker.

**IMP-P15-008:** Email notifications.

**IMP-P15-009:** In-app notifications.

**IMP-P15-010:** Notification preferences.

## Deliverables

- Transaction history API.
- Transaction details.
- Receipts.
- Notification infrastructure.

## Exit Criteria

Customer-visible financial records match authoritative transaction and accounting states.

---

# 29. PHASE 16 — ADMINISTRATIVE AND RECONCILIATION TOOLS

## Objective

Provide operational oversight and controlled financial administration.

## Tasks

**IMP-P16-001:** Administrative authentication.

**IMP-P16-002:** Administrative RBAC.

**IMP-P16-003:** Customer management.

**IMP-P16-004:** KYC review interface.

**IMP-P16-005:** Transaction monitoring.

**IMP-P16-006:** Provider operation monitoring.

**IMP-P16-007:** Reconciliation runs.

**IMP-P16-008:** Reconciliation exception cases.

**IMP-P16-009:** Financial adjustment requests.

**IMP-P16-010:** Maker-checker approvals.

**IMP-P16-011:** Audit log viewer.

**IMP-P16-012:** Emergency financial operation suspension.

## Deliverables

- Administrative dashboard.
- Reconciliation dashboard.
- Controlled approval workflows.
- Audit records.

## Exit Criteria

Authorized operators can investigate financial exceptions without directly editing customer balances.

---

# 30. PHASE 17 — FRONTEND INTEGRATION AND USER EXPERIENCE

## Objective

Deliver the responsive DEYPAY PWA.

The original schedule expected frontend development to proceed alongside backend implementation. The repository owner has approved the account-first scheduling exception below; it supersedes that order for current delivery without redefining any phase or task.

This phase represents final feature integration and user experience completion.

## 30.1 Authentication Screens

- Registration.
- Login.
- Verification.
- Password recovery.

## 30.2 Customer Onboarding

- Profile completion.
- KYC.
- Account status.

## 30.3 Wallet Dashboard

- USDC balance.
- USDT balance.
- Available and reserved amounts.
- Recent transactions.

## 30.4 Send and Receive

- Internal transfer.
- Deposit address.
- External withdrawal.
- Network selection.

## 30.5 Conversion

- Quote.
- Confirmation.
- Processing.
- Completion.

## 30.6 NGN Payments

- Bank verification.
- On-ramp.
- Off-ramp.
- Pay Anyone.

## 30.7 Transaction History

- Filters.
- Details.
- Receipts.

## 30.8 Security

- Session management.
- Step-up authorization.
- Account restrictions.

## 30.9 Frontend Requirements

- Responsive layouts.
- Accessible forms.
- Loading states.
- Error handling.
- Clear financial confirmation.
- No privileged provider secrets.
- No authoritative balance calculation.

### Owner-approved account-first scheduling exception and frontend milestone · 2026-11-25

This is a named milestone within existing Phase 17, not a new task ID, phase or roadmap. It uses existing IMP-P1-004 and the relevant §30.1–2 and §30.8–9 requirements. It moves the first account frontend increment ahead of financial implementation and defers further KYC verification implementation; the original phase definitions and all existing task IDs remain unchanged.

**Milestone: First integrated customer-account frontend.** Begin after the customer-account backend APIs and their dependencies are ready. Reuse the existing Next.js application and integrate registration/email verification, login/password recovery, a non-financial dashboard shell, customer profile-management interfaces, session handling and errors against the real DEYPAY APIs. Include profile-image management only after its API/storage scope and acceptance criteria are approved as a scoped extension to IMP-P3-010; current FRS §9.1 and API §15 do not define it. The customer account and ordinary non-financial account management do not depend on KYC approval.

**Milestone acceptance:** the existing frontend exercises approved real APIs; authenticated requests use the server-side session contract; loading/error states and account workflows are tested; no disconnected mock product endpoints or synthetic balances/completed transactions are presented. This early slice has no new task ID or assignee and does not complete Phase 17. It excludes KYC verification implementation while that work is deferred and does not imply production deployment or financial activation.

**Owner-approved execution schedule and dependency map (2026-11-25):**

1. **Customer-account backend — Phase 3, IMP-P3-001–013.** Complete the existing registration, email verification, login, session, logout, recovery, username, profile, revocation and applicable authentication-security work. Preserve each current status and dependency: IMP-P3-003 phone verification remains Deferred — Post-MVP; IMP-P3-005/006 remain in progress in open PR #9; IMP-P3-009 remains In testing; no unassigned task is assigned by this schedule.
2. **Early account frontend slice — IMP-P1-004 and Phase 17 §30.1, §30.2, relevant §30.8 and §30.9.** After the account APIs are ready, integrate authentication screens, customer onboarding, the dashboard shell and profile-management interfaces using the existing Next.js app and real APIs. This is an early increment, not completion of Phase 17; it adds no task ID. Apply the relevant authentication, session, authorization, privacy and input-handling security checks during implementation. Do not use mock product endpoints or display invented financial state.
3. **Financial backend — Phase 5–16 existing tasks, in dependency order.** Follow the existing technical sequence and task IDs: Phase 5 ledger `IMP-P5-001–017` → Phase 6 wallet/assets `IMP-P6-001–010` → Phase 7 internal transfers `IMP-P7-001–010` → Phase 8 provider framework `IMP-P8-001–014` → Phase 9 deposits `IMP-P9-001–010` and Phase 10 withdrawals `IMP-P10-001–012` → Phase 11 conversion `IMP-P11-001–010` → Phase 12 on-ramp `IMP-P12-001–009` and Phase 13 off-ramp `IMP-P13-001–010` → Phase 14 Pay Anyone `IMP-P14-001–014` → Phase 15 history/notifications `IMP-P15-001–010` → Phase 16 administration/reconciliation `IMP-P16-001–012`. Observe each task's actual data, provider and ledger dependencies; `IMP-P16-004` KYC review interface follows the KYC review workflow `IMP-P4-009`. Apply relevant security, authorization, privacy, provider, concurrency and financial-integrity verification throughout these phases, using the existing Phase 18 criteria rather than waiting until Phase 18 to begin.
4. **Internal transaction validation — existing integration and test tasks.** Validate the real transaction implementation, including applicable `IMP-P14-014`, `IMP-P18-002–008`, `IMP-P18-012` and `IMP-P18-013` criteria, under isolated development/test conditions and authorized provider test environments. Exercise the same implementation intended for deployment. No mock product endpoints, substitute transaction implementations or throwaway infrastructure are permitted.
5. **Resume Phase 4 KYC verification — existing IMP-P4-005–010.** Resume only after internal transaction validation and owner authorization, and complete the applicable KYC, audit, review and provider requirements before live financial activation. IMP-P4-001–004 retain their accepted status; PR #10's IMP-P4-005–007 code remains open/unmerged and on hold; IMP-P4-008–010 remain unassigned. Phase 4 is not complete. While implementation is deferred, live financial activity must fail closed on missing KYC, eligibility or restriction evidence.
6. **External release requirements — existing Phase 18/19 tasks and approvals.** Complete cumulative Phase 18 security/quality exit criteria and Phase 19 `IMP-P19-001–012`, plus applicable legal, regulatory, compliance, provider, partner, operational and reconciliation approvals before any live customer financial service.

This exception overrides the original strict sequencing that placed all Phase 4 work before Phase 5 and Phase 17 after Phase 16. It does not cancel KYC requirements or authorize a financial operation to bypass KYC, account restrictions, approved limits or any other eligibility/compliance control. Transaction implementation and internal testing before KYC resumption must remain fail-closed where approval or policy is absent. Live customer financial execution is prohibited until all applicable gates are cleared. Security verification accompanies each relevant milestone; Phase 18 remains the final integrated assurance gate, not the first point at which security is tested.

## Exit Criteria

All approved MVP customer journeys work against the DEYPAY backend.

---

# 31. PHASE 18 — SECURITY HARDENING AND QUALITY ASSURANCE

## Objective

Verify that the completed system meets the security and testing baseline.

## Tasks

**IMP-P18-001:** Authentication security testing.

**IMP-P18-002:** Authorization testing.

**IMP-P18-003:** Financial invariant testing.

**IMP-P18-004:** Provider failure testing.

**IMP-P18-005:** Webhook security testing.

**IMP-P18-006:** Database concurrency testing.

**IMP-P18-007:** Load testing.

**IMP-P18-008:** Vulnerability scanning.

**IMP-P18-009:** Independent penetration testing.

**IMP-P18-010:** Backup restoration testing.

**IMP-P18-011:** Disaster recovery testing.

**IMP-P18-012:** Full regression testing.

**IMP-P18-013:** Financial reconciliation validation.

## Deliverables

- Test execution reports.
- Security assessment results.
- Remediation records.
- Release readiness report.

## Exit Criteria

No unresolved critical financial or security defects remain.

---

# 32. PHASE 19 — STAGING, PILOT AND PRODUCTION PREPARATION

## Objective

Prepare DEYPAY for controlled launch.

## Tasks

**IMP-P19-001:** Provision production infrastructure.

**IMP-P19-002:** Configure production secrets.

**IMP-P19-003:** Configure production database.

**IMP-P19-004:** Configure monitoring.

**IMP-P19-005:** Configure backups.

**IMP-P19-006:** Configure alerting.

**IMP-P19-007:** Validate provider credentials.

**IMP-P19-008:** Validate legal and regulatory requirements.

**IMP-P19-009:** Conduct production readiness review.

**IMP-P19-010:** Conduct controlled pilot.

**IMP-P19-011:** Reconcile pilot transactions.

**IMP-P19-012:** Approve phased rollout.

## Deliverables

- Production-ready deployment.
- Operational procedures.
- Pilot results.
- Launch approval record.

## Exit Criteria

Technical, financial, security, operational and legal release gates are satisfied.

---

# 33. IMPLEMENTATION DEPENDENCY GRAPH

The owner-approved execution schedule in §30 determines current delivery order. The graph below maps that order to the existing technical phases and task IDs; it does not create tasks or change their acceptance criteria.

```text
REQUIREMENTS / FOUNDATION / DATABASE AND DOMAIN MODELS
                         |
                         v
       PHASE 3 CUSTOMER-ACCOUNT BACKEND
                         |
                         v
      EARLY PHASE 17 ACCOUNT FRONTEND SLICE
                         |
                         v
 PHASE 5 LEDGER -> PHASE 6 WALLET -> PHASE 7 INTERNAL TRANSFERS
                         |
                         v
             PHASE 8 PROVIDER FRAMEWORK
                         |
             +-----------+-----------+
             |                       |
             v                       v
       PHASE 9 DEPOSITS       PHASE 10 WITHDRAWALS
             +-----------+-----------+
                         |
                         v
               PHASE 11 CONVERSION
                         |
             +-----------+-----------+
             |                       |
             v                       v
       PHASE 12 ON-RAMP       PHASE 13 OFF-RAMP
                                     |
                                     v
                            PHASE 14 PAY ANYONE
                                     |
                                     v
       PHASE 15 HISTORY / PHASE 16 OPERATIONS
         (except IMP-P16-004 KYC review)
                                     |
                                     v
                    INTERNAL TRANSACTION VALIDATION
                                     |
                                     v
                 RESUME DEFERRED PHASE 4 KYC WORK
                                     |
                                     v
          IMP-P16-004 KYC REVIEW INTERFACE
          (after IMP-P4-009 review workflow)
                                     |
                                     v
       EXTERNAL RELEASE GATES -> LIVE SERVICE ONLY WHEN CLEARED
```

IMP-P4-001–004 remain accepted foundations. Further KYC verification implementation under IMP-P4-005–010 resumes after internal transaction validation and before live financial activation, subject to owner authorization. The Phase 16 task IMP-P16-004 (KYC review interface) follows the KYC review workflow IMP-P4-009. While Phase 4 verification is deferred, every live financial path remains fail-closed on missing KYC, eligibility or restriction evidence.

Security verification accompanies each relevant milestone, including the applicable IMP-P18-001–008 and IMP-P18-012–013 checks as authentication, frontend, ledger, provider, transaction and reconciliation capabilities are implemented. Phase 18 remains the cumulative system-wide security and quality gate; it is not the first time those controls are tested.

At internal transfer validation, exercise recipient resolution, authorization, posting, participant history, receipts and notifications through the real production-intended services and ledger under isolated conditions. Test fixtures may represent external provider responses only; they must not introduce mock product endpoints or replace transaction implementation. Provider and settlement-route research may proceed in parallel only where it does not bypass implementation dependencies or approval gates.

Work may proceed in parallel within a scheduled milestone when its technical dependencies are satisfied. The early account frontend does not begin before its backend APIs are ready.

---

# 34. SPRINT PLANNING AND DELIVERY CADENCE

## 34.1 Recommended Sprint Duration

Two weeks.

## 34.2 Sprint Planning

Each sprint must define:

- Sprint objective.
- Selected backlog items.
- Dependencies.
- Acceptance criteria.
- Testing requirements.
- Responsible engineers.

## 34.3 Daily Coordination

The team should identify blockers and integration issues regularly.

## 34.4 Sprint Review

Demonstrate completed functionality.

## 34.5 Retrospective

Review delivery problems and process improvements.

## 34.6 Financial Feature Reviews

Critical financial features require additional engineering and accounting review.

---

# 35. INDICATIVE TIMELINE AND MILESTONES

The following is a planning estimate, not a delivery commitment.

It assumes a focused engineering team, timely decisions and reasonable access to required infrastructure.

This indicative timeline does not supersede the owner-approved execution order in §§30 and 33. The account frontend slice is brought forward before Phase 5; the remaining Phase 17 integrations stay subject to their existing dependencies and acceptance criteria.

| Milestone | Indicative Window | Deliverable |
|---|---|---|
| M0 | Weeks 1–2 | Requirements and project foundation |
| M1 | Weeks 3–5 | Database, identity and authentication |
| M2 | Weeks 5–8 | Ledger and wallet foundations |
| M3 | Weeks 8–10 | Internal transfers and provider framework |
| M4 | Weeks 10–14 | Deposits, withdrawals and conversion |
| M5 | Weeks 14–18 | NGN on-ramp and off-ramp |
| M6 | Weeks 17–20 | Pay Anyone and operational tools |
| M7 | Weeks 20–23 | Full integration, security and QA |
| M8 | Weeks 23–26 | Pilot and launch preparation |

## 35.1 Timeline Interpretation

The indicative range is approximately 23–26 weeks for a sufficiently staffed implementation under favorable conditions.

This is not a guarantee.

## 35.2 Schedule Dependencies

The timeline may change due to:

- Team size.
- Provider API quality.
- Regulatory requirements.
- Custody onboarding.
- Banking partnerships.
- Security remediation.
- Financial reconciliation complexity.

## 35.3 Solo Development

A single developer should expect a materially longer schedule.

The project must not sacrifice financial controls to meet an arbitrary deadline.

## 35.4 Milestone Tracking

Each milestone must be assessed by accepted deliverables rather than elapsed time alone.

---

# 36. MINIMUM ENGINEERING TEAM

## 36.1 Recommended Roles

| Role | Primary Responsibility |
|---|---|
| Engineering Lead / Backend Architect | Architecture and financial systems |
| Backend Engineer | Rust APIs and application services |
| Frontend Engineer | Next.js PWA |
| QA Engineer | Automated and manual testing |
| DevOps Engineer | Deployment and infrastructure |
| Security Reviewer | Security assessment |
| Compliance / Legal Advisor | Regulatory operating model |
| Financial Operations Reviewer | Ledger and reconciliation |

## 36.2 Lean Team Model

During early development, some engineering responsibilities may be combined.

However, independent review must be retained for critical security and financial decisions.

## 36.3 External Specialists

Independent legal, security and financial control expertise may be required.

---

# 37. WORK BREAKDOWN AND TASK MANAGEMENT

## 37.1 Task Structure

Each implementation task must contain:

- Task ID.
- Title.
- Description.
- Related requirement.
- Dependencies.
- Priority.
- Assigned owner.
- Acceptance criteria.
- Test requirements.
- Status.

## 37.2 Priority Levels

- P0 — Critical foundation or financial integrity.
- P1 — Required MVP.
- P2 — Important improvement.
- P3 — Future enhancement.

## 37.3 Status Values

- Backlog.
- Ready.
- In progress.
- In review.
- In testing.
- Blocked.
- Deferred — Post-MVP.
- Completed.

## 37.4 Dependency Tracking

Blocked tasks must identify the blocking decision or external dependency.

---

# 38. DEFINITION OF READY

A task is ready for implementation when:

1. Its purpose is clear.
2. Its requirements are documented.
3. Its dependencies are understood.
4. Its acceptance criteria are defined.
5. Its API or data contract is sufficiently specified.
6. Required test cases are identified.
7. External assumptions are recorded.
8. Security implications are understood.

Financial tasks additionally require approved accounting behavior.

---

# 39. DEFINITION OF DONE

A task is complete when:

1. Implementation satisfies requirements.
2. Code review is complete.
3. Unit tests pass.
4. Integration tests pass where applicable.
5. API contracts are updated.
6. Database migrations are validated.
7. Security requirements are enforced.
8. Logs and errors are appropriate.
9. Documentation is updated.
10. Acceptance criteria are verified.

Financial features additionally require:

11. Ledger invariants pass.
12. Idempotency is verified.
13. Failure paths are tested.
14. Reconciliation behavior is validated.
15. No unresolved critical financial defect remains.

---

# 40. DATABASE MIGRATION AND DATA GOVERNANCE

## 40.1 Migration Tooling

Use SQLx-compatible migrations.

## 40.2 Migration Review

Schema changes require review.

## 40.3 Financial Tables

Financial table changes require additional scrutiny.

## 40.4 Production Migration Safety

Avoid destructive changes without a recovery plan.

## 40.5 Backward Compatibility

Application and database deployments must support the approved rollout strategy.

## 40.6 Data Retention

Retention policies must be implemented according to legal and operational requirements.

## 40.7 Backup Validation

Backups must be restorable.

---

# 41. API CONTRACT IMPLEMENTATION

## 41.1 Contract-First Development

Public API schemas should be finalized before frontend integration.

## 41.2 OpenAPI

Maintain machine-readable API documentation.

## 41.3 Typed DTOs

Rust request and response types must align with approved contracts.

## 41.4 Frontend Client

Generate or maintain a typed frontend API client.

## 41.5 Error Consistency

Use standardized error codes.

## 41.6 Financial Amounts

Never use floating-point numbers for authoritative financial calculations.

## 41.7 Versioning

Breaking changes require explicit version management.

---

# 42. PROVIDER INTEGRATION AND NUVION VALIDATION

## 42.1 Strategic Objective

Use Nuvion only for the capabilities it can reliably and legally provide.

DEYPAY must retain control of the customer application and internal financial logic.

## 42.2 Required Validation

Obtain written confirmation of:

1. Nigerian customer support.
2. Nigerian retail KYC requirements.
3. Custody operating model.
4. Supported stablecoins.
5. Supported blockchain networks.
6. Deposit address provisioning.
7. Blockchain withdrawals.
8. USDC/USDT conversion.
9. NGN funding.
10. NGN bank payouts.
11. Third-party bank payouts.
12. Bank account verification.
13. Webhook capabilities.
14. Provider transaction lookup.
15. Idempotency behavior.
16. Fees.
17. Settlement timing.
18. Production onboarding requirements.
19. Regulatory responsibilities.
20. Incident and recovery support.

## 42.3 Capability Classification

Each capability must be classified:

- Confirmed.
- Confirmed with restrictions.
- Unverified.
- Unsupported.

## 42.4 Integration Policy

Only confirmed capabilities may be enabled in production.

Implement Pay Anyone against a simulator or approved sandbox while route evidence is pending. Gate real-money activation on DEC-009, including written confirmation covering third-party recipients and the parties accountable for settlement and recovery.

## 42.5 Adapter Testing

Each provider adapter must pass contract tests.

## 42.6 Provider Exit Strategy

Provider references must remain separate from DEYPAY customer and transaction identifiers.

## 42.7 Alternative Providers

Alternative providers may be evaluated for unsupported capabilities.

Provider selection must be based on legal suitability, technical reliability, pricing and operational support.

---

# 43. FINANCIAL INTEGRITY AND RECONCILIATION CONTROLS

## 43.1 Ledger Authority

The DEYPAY ledger is the authoritative source for customer balances and financial obligations.

## 43.2 Provider Evidence

Provider balances and events are external evidence requiring reconciliation.

## 43.3 Reconciliation Scope

Reconcile:

- Custody balances.
- Blockchain deposits.
- Blockchain withdrawals.
- FX conversions.
- NGN collections.
- NGN payouts.
- Treasury prefunding.
- Fees.
- Refunds.
- Reversals.

## 43.4 Reconciliation Frequency

Critical reconciliation must run at an approved operational frequency.

## 43.5 Exceptions

Discrepancies must generate investigation records.

## 43.6 Financial Adjustments

Adjustments must use approved accounting workflows.

## 43.7 Prohibited Operations

No direct manual balance editing.

No deletion of posted journals.

No blind retry of uncertain external payouts.

---

# 44. SECURITY AND COMPLIANCE IMPLEMENTATION GATES

## Gate SEC-01 — Identity Security

Authentication and session controls implemented.

## Gate SEC-02 — Financial Authorization

Transaction-specific authorization implemented.

## Gate SEC-03 — Provider Security

Provider credentials and webhook authentication protected.

## Gate SEC-04 — Ledger Security

Financial posting permissions and immutability enforced.

## Gate SEC-05 — Administrative Security

RBAC and maker-checker implemented.

## Gate SEC-06 — Data Protection

Sensitive data protection controls implemented.

## Gate SEC-07 — Independent Assessment

Required security assessment completed.

## Gate SEC-08 — Regulatory Approval

Applicable legal and regulatory launch requirements satisfied.

---

# 45. TESTING AND QUALITY GATES

## 45.1 Unit Tests

Required for domain rules.

## 45.2 Integration Tests

Required for database and service interactions.

## 45.3 Financial Invariant Tests

Required for ledger and payment operations.

## 45.4 API Contract Tests

Required for public endpoints.

## 45.5 End-to-End Tests

Required for critical customer journeys.

## 45.6 Provider Failure Tests

Required for external financial operations.

## 45.7 Security Tests

Required for authentication, authorization and financial security.

## 45.8 Regression Tests

Required before production releases.

## 45.9 Release Blocking

Critical financial integrity defects block release.

---

# 46. CI/CD AND DEPLOYMENT STRATEGY

## 46.1 Continuous Integration

Run automated checks for every relevant pull request.

## 46.2 Build Pipeline

Build Rust backend and frontend artifacts.

## 46.3 Containerization

Package backend services into approved container images.

## 46.4 Staging Deployment

Deploy approved builds to staging.

## 46.5 Production Approval

Production releases require authorization.

## 46.6 Database Migrations

Migrations must follow controlled procedures.

## 46.7 Rollback

Application rollback must not reverse completed financial settlement.

## 46.8 Feature Flags

Provider-dependent financial capabilities must be controlled by feature flags.

---

# 47. OBSERVABILITY AND OPERATIONAL READINESS

## 47.1 Logging

Use structured logs.

## 47.2 Metrics

Monitor:

- API latency.
- Error rate.
- Worker health.
- Job queue depth.
- Provider failures.
- Pending transactions.
- Reconciliation discrepancies.
- Financial posting failures.

## 47.3 Alerting

Critical financial and security events must generate alerts.

## 47.4 Audit Logs

Administrative and financial actions must be auditable.

## 47.5 Incident Response

Document incident escalation and recovery.

## 47.6 Runbooks

Operational procedures must cover:

- Provider outage.
- Database outage.
- Stuck payment.
- Failed payout.
- Reconciliation discrepancy.
- Security incident.
- Backup restoration.

---

# 48. RELEASE MANAGEMENT AND FEATURE FLAGS

## 48.1 Release Categories

- Development release.
- Internal testing release.
- Staging release.
- Controlled pilot.
- Production release.

## 48.2 Financial Feature Flags

Recommended flags:

- Stablecoin deposits.
- External withdrawals.
- Internal transfers.
- Conversion.
- NGN on-ramp.
- NGN off-ramp.
- Pay Anyone.

## 48.3 Emergency Controls

The platform must support suspension of affected financial operations.

## 48.4 Progressive Rollout

Enable features gradually where operationally appropriate.

## 48.5 Rollout Monitoring

Monitor financial and technical indicators during rollout.

---

# 49. BUDGET AND INFRASTRUCTURE COST MANAGEMENT

## 49.1 Cost Principle

Begin with infrastructure that is sufficient for safe development and early production operations.

## 49.2 Cost Categories

- Hosting.
- Managed PostgreSQL.
- Storage.
- Backups.
- Monitoring.
- Email and SMS.
- KYC verification.
- Custody services.
- Conversion fees.
- Banking and payout fees.
- Security testing.
- Compliance and legal services.

## 49.3 Avoid Premature Spending

Do not introduce expensive infrastructure without a demonstrated requirement.

## 49.4 Cost Monitoring

Track infrastructure and provider costs.

## 49.5 Financial Operations Costs

Provider fees and settlement costs must be incorporated into the business model.

## 49.6 Budget Approval

Production infrastructure commitments must be approved before purchase.

---

# 50. RISK REGISTER AND MITIGATION PLAN

| ID | Risk | Severity | Mitigation |
|---|---|---|---|
| R-001 | Nuvion lacks required Nigeria capabilities | Critical | Written confirmation and alternative provider strategy |
| R-002 | Regulatory model is not approved | Critical | Legal review before financial launch |
| R-003 | Ledger accounting defect | Critical | Invariant tests and financial review |
| R-004 | Duplicate financial execution | Critical | Idempotency and reconciliation |
| R-005 | Unauthorized withdrawal | Critical | Strong authorization and risk controls |
| R-006 | Provider outage | High | Safe states and recovery |
| R-007 | Delayed banking integration | High | Parallel provider validation |
| R-008 | Insufficient engineering capacity | High | Prioritized scope and realistic scheduling |
| R-009 | Security vulnerabilities | Critical | Secure development and independent assessment |
| R-010 | Infrastructure overspending | Medium | Cost monitoring and lean deployment |
| R-011 | Incomplete test coverage | High | Mandatory test gates |
| R-012 | Inconsistent documentation | Medium | Baseline review and change control |
| R-013 | Treasury liquidity shortage | High | Liquidity planning and monitoring |
| R-014 | Failed payout recovery complexity | High | Explicit clearing and refund workflows |
| R-015 | Production reconciliation failure | Critical | Independent reconciliation and exception handling |

---

# 51. DEPENDENCY AND DECISION REGISTER

The following decisions must be tracked.

| ID | Decision | Dependency | Status |
|---|---|---|---|
| DEC-001 | Production hosting provider | Infrastructure evaluation | Open |
| DEC-002 | PWA session lifetimes, step-up method and recovery hold duration | Security and risk approval | Open |
| DEC-003 | Supported blockchain networks | Provider confirmation | Open |
| DEC-004 | Custody arrangement | Legal and provider review | Open |
| DEC-005 | Nuvion capability scope | Written provider confirmation | Open |
| DEC-006 | KYC provider | Compliance requirements | Open |
| DEC-007 | NGN collection provider | Banking and legal review | Open |
| DEC-008 | NGN payout provider | Banking and legal review | Open |
| DEC-009 | Approve the live Pay Anyone settlement route, entities, accounts, permitted recipients and responsibilities | Legal, Compliance, Operations and contracting partners | Open for live activation |
| DEC-010 | Fee schedule | Commercial approval | Open |
| DEC-011 | Limit thresholds, windows and cross-asset valuation | Risk and compliance policy | Open |
| DEC-012 | Production recovery objectives | Operations review | Open |
| DEC-013 | Security assessment provider | Procurement and security review | Open |
| DEC-014 | Pilot launch scope | Product and operations approval | Open |
| DEC-015 | Failed-payout refund denomination, fee treatment and FX gain/loss allocation | Finance and Product, with partner settlement terms | Open for live recovery |

Decisions must not be marked confirmed without supporting evidence.

---

# 52. ACCEPTANCE CRITERIA BY MILESTONE

| Milestone | Required Acceptance |
|---|---|
| M0 | Requirements and architecture baseline reviewed |
| M1 | Development environment and identity services functional |
| M2 | Ledger invariants and wallet balances validated |
| M3 | Simulated internal transfer, beneficiary management and provider framework tested (test §§19, 25, 27; E2E-007–009). |
| M4 | Stablecoin deposits, withdrawals and conversion validated |
| M5 | NGN collection and payout workflows tested |
| M6 | Pay Anyone, history, notifications, audit/reporting, reconciliation and support workflows validated (test §§24, 26, 36; E2E-016–019, 023). |
| M7 | PWA, QR payments and risk-controlled links pass E2E-021–022; customer support passes E2E-023 through the PWA. Security, regression and recovery tests passed. |
| M8 | Pilot, reconciliation and production approvals completed |

A milestone must not be marked complete merely because code has been written.

---

# 53. MVP LAUNCH READINESS CHECKLIST

## 53.1 Product

- [ ] MVP features implemented.
- [ ] Customer journeys tested.
- [ ] User-facing fees disclosed.
- [ ] Error handling reviewed.
- [ ] Customer support procedures established.

## 53.2 Engineering

- [ ] Backend builds successfully.
- [ ] Frontend builds successfully.
- [ ] Database migrations validated.
- [ ] API contracts tested.
- [ ] Worker processing validated.
- [ ] Monitoring operational.

## 53.3 Financial

- [ ] Ledger invariants pass.
- [ ] Balance projections verified.
- [ ] Deposit reconciliation validated.
- [ ] Withdrawal reconciliation validated.
- [ ] Conversion accounting validated.
- [ ] NGN payout reconciliation validated.
- [ ] Refund and reversal procedures validated.

## 53.4 Security

- [ ] Authentication controls tested.
- [ ] Authorization controls tested.
- [ ] Step-up authorization tested.
- [ ] Provider credentials protected.
- [ ] Webhooks verified.
- [ ] Administrative permissions reviewed.
- [ ] Independent security assessment completed.

## 53.5 Compliance

- [ ] Applicable Nigerian legal review completed.
- [ ] Custody arrangement approved.
- [ ] KYC requirements established.
- [ ] AML/CFT controls approved.
- [ ] Provider contractual responsibilities confirmed.
- [ ] Pay Anyone permissions confirmed before activation.

## 53.6 Operations

- [ ] Backups configured.
- [ ] Restore test completed.
- [ ] Incident response procedures documented.
- [ ] Provider escalation contacts established.
- [ ] Reconciliation procedures operational.
- [ ] Treasury monitoring operational.

## 53.7 Launch Approval

- [ ] Product approval.
- [ ] Engineering approval.
- [ ] Security approval.
- [ ] Financial operations approval.
- [ ] Compliance approval.
- [ ] Legal approval.
- [ ] Operations approval.

---

# 54. POST-LAUNCH STABILIZATION AND SCALING

## 54.1 Stabilization Period

A controlled stabilization period should follow initial production launch.

## 54.2 Monitoring Priorities

Monitor:

- Transaction success rates.
- Provider latency.
- Financial discrepancies.
- Customer support issues.
- Security alerts.
- Infrastructure utilization.
- Conversion performance.
- Payout failures.

## 54.3 Incident Review

Material incidents must result in corrective actions.

## 54.4 Performance Optimization

Optimize based on measured bottlenecks.

## 54.5 Infrastructure Scaling

Scale API and worker capacity independently where appropriate.

## 54.6 Future Mobile Applications

Native mobile applications should reuse the existing DEYPAY API contracts.

## 54.7 Future Provider Expansion

Additional providers must integrate through the established adapter interfaces.

## 54.8 Product Expansion

Future financial products require new requirements, accounting design, risk assessment and testing before release.

---

# 55. IMMEDIATE ENGINEERING ACTION PLAN

This section defines the first practical actions after approval of the eight-document baseline.

## Step 1 — Finalize Documentation Consistency

Review the eight documents for:

- Numbering.
- Cross-references.
- API endpoint consistency.
- Financial state definitions.
- Ledger posting rules.
- Provider assumptions.
- Security requirements.

**Expected Output:** Consistent engineering baseline.

## Step 2 — Establish Repository

Create the DEYPAY monorepo.

Add the eight approved documents under `/docs`.

**Expected Output:** Version-controlled project repository.

## Step 3 — Initialize Rust Workspace

Create:

- `deypay-domain`
- `deypay-application`
- `deypay-infrastructure`
- `deypay-api`
- `deypay-worker`
- `deypay-common`

**Expected Output:** Compiling Rust workspace.

## Step 4 — Initialize Next.js Frontend

Create the responsive frontend application.

**Expected Output:** Running frontend development server.

## Step 5 — Configure PostgreSQL

Create local database services and migration tooling.

**Expected Output:** Working PostgreSQL connection and migrations.

## Step 6 — Implement Foundational Domain Types

Start with:

- User IDs.
- Wallet IDs.
- Asset IDs.
- Exact financial amounts.
- Transaction IDs.
- Domain errors.
- Transaction states.

**Expected Output:** Tested shared domain foundation.

## Step 7 — Implement Authentication

Build registration, login, verification and session management.

**Expected Output:** Secure customer identity foundation.

## Step 8 — Implement Ledger Core

Build immutable journal posting, balance projections and reservations.

**Expected Output:** Tested financial accounting engine.

## Step 9 — Implement Wallet Profiles

Create USDC and USDT wallet profiles backed by the ledger.

**Expected Output:** Wallet balance APIs.

## Step 10 — Implement Internal Transfers

Build the first complete financial workflow without depending on external settlement.

**Expected Output:** Atomic, idempotent internal transfers.

## Step 11 — Validate Nuvion

Obtain current documentation, sandbox access and written capability confirmation.

**Expected Output:** Confirmed provider integration matrix.

## Step 12 — Implement Provider Adapters

Build provider interfaces, mocks, webhook infrastructure and confirmed Nuvion operations.

**Expected Output:** Testable provider integration layer.

## Step 13 — Implement External Financial Operations

Proceed with deposits, withdrawals, conversions and approved NGN payment flows.

**Expected Output:** Reconciled external financial workflows.

## Step 14 — Complete Frontend Integration

Connect all approved customer journeys to backend APIs.

**Expected Output:** Functional MVP PWA.

## Step 15 — Complete Production Validation

Execute security, financial, integration and recovery testing.

**Expected Output:** Evidence-based release readiness assessment.

---

# 56. FINAL IMPLEMENTATION STRATEGY AND APPROVAL

## 56.1 Final Engineering Strategy

DEYPAY will be implemented as a modular monolith using Rust, Axum, Tokio, SQLx and PostgreSQL.

The frontend will be developed using Next.js, React and TypeScript.

The application will initially support USDC and USDT customer wallets.

The financial ledger will be independent of external custody providers.

Customer balances will be derived from immutable double-entry accounting records.

Financial operations will use exact monetary representations, durable idempotency, controlled authorization and explicit transaction states.

External infrastructure will be integrated through internal provider adapters.

Nuvion will be considered for confirmed and approved capabilities.

NGN payment functionality will follow the selected Model B operating approach without introducing a general-purpose customer NGN stored-value wallet.

Pay Anyone will enable eligible customers to fund approved third-party Nigerian bank payouts using supported stablecoins, subject to legal and provider authorization.

The implementation will proceed through controlled milestones with automated testing and security review.

Production financial operations will not be enabled until the required technical, financial, operational, provider and legal conditions are satisfied.

## 56.2 Final Execution Principle

**Build the foundations before the financial features.**

**Build the ledger before moving customer funds.**

**Build authorization before allowing withdrawals.**

**Build provider adapters before depending on external infrastructure.**

**Build reconciliation before enabling live settlement.**

**Test failure conditions before production launch.**

**Verify legal and operational readiness before handling real customer money.**

## 56.3 Project Completion Definition

The DEYPAY MVP will be considered implementation-complete when:

1. Approved MVP functionality is implemented.
2. Required APIs are available.
3. Customer authentication and KYC controls work.
4. Wallet balances are ledger-backed.
5. Internal transfers are atomic.
6. External financial workflows are integrated.
7. NGN payment capabilities are validated.
8. Pay Anyone is implemented against approved infrastructure.
9. Administrative controls are operational.
10. Financial reconciliation is functioning.
11. Critical tests pass.
12. Security assessment requirements are satisfied.
13. Operational recovery procedures are validated.
14. Documentation matches the implementation.

Implementation completion does not automatically mean production launch authorization.

Production launch requires separate approval.

---

# DOCUMENT APPROVAL

| Role | Responsibility | Status |
|---|---|---|
| Product Owner | Business and scope approval | Pending |
| Engineering Lead | Technical implementation approval | Pending |
| Backend Lead | Financial and backend design approval | Pending |
| Frontend Lead | User application approval | Pending |
| QA Lead | Testing strategy alignment | Pending |
| Security Lead | Security architecture approval | Pending |
| Financial Operations Lead | Ledger and reconciliation approval | Pending |
| Compliance Lead | Compliance approval | Pending |
| Legal Counsel | Regulatory and contractual review | Pending |

**Document ID:** DP-IMP-008  
**Version:** 1.0  
**Status:** Proposed Engineering Execution Baseline  
**Filename:** `08-IMPLEMENTATION-PLAN.md`

---

# END OF DOCUMENT

**DEYPAY SOFTWARE IMPLEMENTATION PLAN AND ENGINEERING EXECUTION ROADMAP**

**KENULE AFRICA — DEYPAY PROJECT**

**Version 1.0 — 18 November 2026**
