# DEYPAY (DP)
# SOFTWARE TEST STRATEGY AND QUALITY ASSURANCE SPECIFICATION

**Document ID:** DP-TEST-007  
**Version:** 1.1  
**Status:** Approved Engineering Baseline  
**Approval Status:** Approved  
**Approval Authority:** DEYPAY Project Owner  
**Approval Date:** 26 November 2026  
**Project:** DEYPAY  
**Organization:** Kenule Africa  
**Document Type:** Software Test Strategy and Quality Assurance Specification  
**Filename:** `07-TEST-STRATEGY.md`  
**Classification:** Confidential — Internal Engineering

---

# DOCUMENT CONTROL

| Field | Description |
|---|---|
| Document Title | DEYPAY Software Test Strategy and Quality Assurance Specification |
| Document ID | DP-TEST-007 |
| Version | 1.1 |
| Product | DEYPAY |
| Organization | Kenule Africa |
| Primary Market | Nigeria |
| Backend | Rust, Axum, Tokio, SQLx |
| Frontend | Next.js, React, TypeScript |
| Database | PostgreSQL |
| Architecture | Modular Monolith |
| Primary CI Platform | GitHub Actions |
| Local Testing Policy | Lightweight and environment-specific checks |
| Initial Customer Assets | USDC and USDT |
| Fiat Settlement Currency | NGN |
| Selected Infrastructure Provider | Nuvion |
| Initial Blockchain | Solana |
| Document Status | Approved Engineering Baseline |
| Approval Status | Approved |
| Approval Date | 26 November 2026 |

## DOCUMENT PURPOSE

This document establishes the testing, validation, verification, quality assurance and engineering acceptance standards for DEYPAY.

**GitHub Actions is the primary execution environment for computationally expensive automated testing.**

The objective is to reduce local machine resource consumption while maintaining production-grade software quality, financial integrity, security and recoverability.

Claude, Codex and all future engineering agents must use GitHub-hosted CI infrastructure wherever technically feasible.

The strategy covers:

- Backend and frontend testing.
- Authentication and session security.
- PostgreSQL integration.
- API contracts.
- Financial ledger integrity.
- Wallet operations.
- Blockchain deposits and withdrawals.
- On-ramping and off-ramping.
- Nuvion integration.
- Auto-Settlement.
- Internal transfers and bank payments.
- Concurrency and idempotency.
- Security and recovery.
- Performance and reliability.
- Production readiness.

This document defines requirements and acceptance standards. Its approval does not constitute evidence that individual software features have passed testing.

---

# TABLE OF CONTENTS

1. Executive Summary
2. Testing Objectives
3. Testing Principles
4. Scope of Testing
5. Testing Governance and Responsibilities
6. Test Architecture
7. Testing Environments
8. Test Data Management
9. Unit Testing Strategy
10. Integration Testing Strategy
11. API Testing Strategy
12. Database Testing Strategy
13. Financial Ledger Testing
14. Customer Registration Testing
15. Authentication and Session Testing
16. KYC and Compliance Testing
17. Wallet Management Testing
18. Stablecoin Deposit Testing
19. Internal Transfer Testing
20. External Withdrawal Testing
21. Stablecoin Conversion Testing
22. NGN On-Ramp Testing
23. NGN Off-Ramp Testing
24. Pay Anyone Testing
25. Beneficiary and Bank Verification Testing
26. Transaction History and Receipt Testing
27. Provider Integration Testing
28. Webhook Testing
29. Idempotency and Concurrency Testing
30. Transaction State Machine Testing
31. Frontend and User Experience Testing
32. Security Testing
33. Fraud and Abuse Testing
34. Performance and Load Testing
35. Reliability and Resilience Testing
36. Disaster Recovery Testing
37. Accessibility and Compatibility Testing
38. Regulatory and Compliance Validation
39. End-to-End Testing
40. Regression Testing
41. Automated Testing and CI/CD
42. Defect Management
43. Test Coverage and Traceability
44. Test Execution and Reporting
45. Release Entry and Exit Criteria
46. Production Readiness and Sign-Off
47. Post-Deployment Validation
48. Test Risks and Mitigations
49. Testing Deliverables
50. Acceptance Criteria and Final Definition
51. Mandatory GitHub Actions Execution Policy
52. Agent CI Monitoring Protocol
53. Local Testing and Environment-Specific Acceptance
54. CI Security, Performance and Maintenance Rules

---

# 1. EXECUTIVE SUMMARY

DEYPAY is a stablecoin-first financial platform designed initially for Nigeria.

The product will support USDC and USDT wallets, external blockchain deposits and withdrawals, internal transfers, stablecoin conversion, NGN funding, bank payouts and Auto-Settlement.

Testing must go beyond ordinary functional verification because the product will handle financial value.

Critical risks include:

- Incorrect customer balances.
- Duplicate credits.
- Double spending.
- Duplicate payouts.
- Unbalanced journals.
- Unauthorized withdrawals.
- Incorrect conversion amounts.
- Provider settlement inconsistencies.
- Failed transaction recovery.
- Sensitive information exposure.

DEYPAY adopts a risk-based, automation-first testing approach.

## 1.1 GitHub-First Testing Decision

GitHub Actions is the primary automated testing platform.

Heavy builds, compilation, static analysis, database integration tests and regression suites must run on GitHub-hosted runners wherever feasible.

Local development machines should primarily handle editing, lightweight checks, interactive debugging and environment-specific verification.

**A test result is valid evidence only when its completed GitHub Actions run matches the exact code revision under review.**

---

# 2. TESTING OBJECTIVES

**TEST-OBJ-001 — Functional Correctness**

Verify that implemented functionality satisfies approved requirements.

**TEST-OBJ-002 — Financial Integrity**

Verify financial balances, reservations and journals.

**TEST-OBJ-003 — Security**

Verify authentication, authorization and sensitive information protection.

**TEST-OBJ-004 — Provider Reliability**

Verify successful, failed, delayed and uncertain provider operations.

**TEST-OBJ-005 — Data Integrity**

Verify consistent database and accounting records.

**TEST-OBJ-006 — Transaction Safety**

Prevent double spending, duplicate requests and unsafe retries.

**TEST-OBJ-007 — Performance**

Measure system behavior against approved thresholds.

**TEST-OBJ-008 — Recoverability**

Verify failures cannot silently lose or duplicate financial value.

**TEST-OBJ-009 — Compliance Enforcement**

Verify applicable customer eligibility and transaction restrictions.

**TEST-OBJ-010 — Release Confidence**

Collect objective engineering acceptance evidence.

**TEST-OBJ-011 — Efficient Development**

Reduce unnecessary local computation through remote CI verification.

**TEST-OBJ-012 — Agent Accountability**

Require engineering agents to report exact commits, workflow results and unresolved test failures.

---

# 3. TESTING PRINCIPLES

## 3.1 Financial Correctness First

Financial correctness takes precedence over performance and convenience.

## 3.2 Test Early

Testing begins during implementation rather than after development finishes.

## 3.3 GitHub Actions First

Heavy automated testing must run on GitHub-hosted infrastructure wherever possible.

## 3.4 Deterministic Testing

Automated tests must use reproducible configurations and controlled data.

## 3.5 Independent Verification

Critical financial outcomes must be checked against independent records.

## 3.6 Failure-Condition Coverage

Every critical financial operation must be tested for success, failure, concurrency and recovery.

## 3.7 Real Infrastructure Where Required

PostgreSQL integration tests must use real PostgreSQL.

Provider simulations do not replace real provider acceptance.

## 3.8 No Real Customer Funds in Automated Testing

Ordinary CI must not move real financial assets.

## 3.9 Evidence-Based Approval

Agents must provide verifiable test evidence.

## 3.10 Latest-Commit Verification

Passing tests from older commits do not establish that newer commits are correct.

---

# 4. SCOPE OF TESTING

## 4.1 In-Scope Components

- Customer registration and authentication.
- Session management and password recovery.
- Customer profiles and usernames.
- Wallet provisioning and balances.
- Financial ledger.
- Blockchain deposits and withdrawals.
- Internal transfers.
- Stablecoin conversion.
- NGN on-ramping.
- NGN off-ramping.
- Auto-Settlement.
- Pay Anyone.
- Beneficiary and bank verification.
- Nuvion integration.
- Solana blockchain integration.
- RPC infrastructure.
- Webhook processing.
- Transaction history and receipts.
- Administration and reconciliation.
- Frontend application.
- Security controls.
- CI/CD and deployment verification.

## 4.2 Future Features

The following require separate implementation and testing scope:

- Polymarket.
- Polygon trading.
- Debit cards.
- Gift cards.
- Additional blockchain networks.
- Other future financial products.

Debit cards and gift cards are planned together as future integrations.

## 4.3 Scope Changes

New functionality must receive corresponding test requirements and acceptance criteria before being considered complete.

---

# 5. TESTING GOVERNANCE AND RESPONSIBILITIES

## 5.1 Product Owner

Controls project scope, implementation authorization, milestones and final product acceptance.

## 5.2 Engineering Agents

Claude, Codex and future engineering agents must:

1. Implement assigned production functionality.
2. Add appropriate tests.
3. Commit and push to assigned branches.
4. Monitor GitHub Actions.
5. Investigate failed jobs.
6. Fix implementation defects.
7. Verify the corrected commit.
8. Report actual test results.

Agents must not modify unrelated ownership areas without authorization.

## 5.3 Engineering Review

Review implementation quality, architecture, CI evidence and security implications.

## 5.4 Security Review

Assess critical authentication and financial security boundaries.

## 5.5 Financial Operations Review

Validate ledger, settlement and reconciliation correctness.

## 5.6 Approval Authority

The DEYPAY Project Owner approves this testing strategy for internal development.

Approval of this document does not claim that external audits, certifications or regulatory permissions have been independently obtained.

---

# 6. TEST ARCHITECTURE

DEYPAY uses a layered testing model:

    MANUAL ACCEPTANCE
            |
      END-TO-END TESTS
            |
      API CONTRACT TESTS
            |
      INTEGRATION TESTS
            |
         UNIT TESTS

## 6.1 Unit Tests

Verify isolated business logic.

## 6.2 Integration Tests

Verify application and infrastructure interactions.

## 6.3 API Tests

Verify HTTP routes, authentication, authorization and contracts.

## 6.4 End-to-End Tests

Verify complete customer workflows.

## 6.5 Security Tests

Verify protection and abuse resistance.

## 6.6 Performance Tests

Verify capacity and response behavior.

## 6.7 Recovery Tests

Verify safe operation following failures.

## 6.8 Execution Platform

Automated tests run through GitHub Actions by default.

Environment-specific checks may run locally or on separately authorized infrastructure.

---

# 7. TESTING ENVIRONMENTS

## 7.1 Local Development

Used for:

- Editing.
- Interactive debugging.
- Targeted tests.
- Manual frontend inspection.
- Private environment-specific verification.

Avoid unnecessary repeated full-workspace testing.

## 7.2 GitHub Actions

Primary automated testing environment.

GitHub-hosted runners should execute:

- Rust formatting.
- Clippy.
- Rust compilation.
- Unit tests.
- PostgreSQL integration tests.
- Database migrations.
- HTTP integration tests.
- Backend lifecycle tests.
- Frontend linting.
- TypeScript checks.
- Frontend builds.
- Frontend unit tests.
- Financial invariant tests.
- Applicable regression tests.

CI resources must be isolated and cleaned up afterward.

## 7.3 Integration Environment

Supports the Rust API, workers, PostgreSQL, provider adapters, webhook receiver and transaction processing.

## 7.4 Staging Environment

Supports production-like tests, including real email delivery, provider sandbox access and HTTPS authentication.

Staging requires separate credentials and configuration.

## 7.5 Production

Only approved smoke tests and authorized operational verification may run in production.

---

# 8. TEST DATA MANAGEMENT

## 8.1 Synthetic Customer Data

Use controlled fictional identities.

## 8.2 Synthetic Financial Data

Use reproducible test balances and transactions.

## 8.3 Provider Test Data

Use authorized sandbox resources where available.

## 8.4 Data Isolation

CI must not connect to production customer databases.

## 8.5 Sensitive Data

Never expose credentials, passwords, API keys, session tokens or private configuration in logs.

## 8.6 Fixtures

Support:

- Verified and unverified accounts.
- Restricted accounts.
- Sufficient and insufficient funds.
- Reserved balances.
- Duplicate requests.
- Concurrent requests.
- Failed provider operations.
- Pending reconciliation.
- Expired and revoked sessions.

## 8.7 Cleanup

Temporary databases, services and test records must be isolated or removed following execution.

---

# 9. UNIT TESTING STRATEGY

## 9.1 Scope

Test:

- Financial amount validation.
- Asset precision.
- Username rules.
- Password requirements.
- Quote expiration.
- Fee calculations.
- Authorization.
- Transaction states.
- Ledger balancing.

## 9.2 Rust Tooling

Required commands, executed from the backend directory:

    cargo fmt --all -- --check
    cargo clippy --locked --workspace --all-targets -- -D warnings
    cargo test --locked --workspace

## 9.3 Requirements

Tests must be deterministic, isolated, repeatable and clearly named.

Financial calculations must use exact representations.

---

# 10. INTEGRATION TESTING STRATEGY

## 10.1 Required Integrations

- API to PostgreSQL.
- Authentication to sessions.
- Application services to ledger.
- Ledger to balance projections.
- Payments to provider adapters.
- Workers to durable jobs.
- Webhooks to event inbox.
- Reconciliation to provider records.

## 10.2 Database

Use real PostgreSQL for financial integration tests.

## 10.3 Rollback

Failed operations must not persist partial financial state.

## 10.4 CI

Run reproducible database integration tests against disposable PostgreSQL in GitHub Actions.

---

# 11. API TESTING STRATEGY

## 11.1 Contracts

Verify implemented routes against approved API specifications.

## 11.2 Coverage

- HTTP methods and paths.
- Request validation.
- Response schemas.
- Authentication.
- Authorization.
- Error handling.
- Pagination.
- Idempotency.
- Rate limiting.

## 11.3 Negative Tests

Test malformed requests, invalid amounts, expired quotes, unauthorized access and conflicting idempotency keys.

## 11.4 Compatibility

Approved clients must not break from unintended contract changes.

---

# 12. DATABASE TESTING STRATEGY

## 12.1 Migrations

Test clean installation and supported upgrade paths.

## 12.2 Constraints

Verify primary keys, foreign keys, unique constraints and financial integrity rules.

## 12.3 Concurrency

Test locking behavior and simultaneous operations.

## 12.4 Recovery

Validate supported rollback and recovery procedures.

## 12.5 CI

Provision isolated PostgreSQL, apply migrations and run database tests through GitHub Actions.

---

# 13. FINANCIAL LEDGER TESTING

## 13.1 Double-Entry Validation

Every journal balances independently by asset.

## 13.2 Immutability

Posted entries cannot be altered or deleted.

## 13.3 Available Balances

Available balances must remain correct.

## 13.4 Reserved Balances

Reservations must remain accurate.

## 13.5 Insufficient Funds

Spending cannot exceed available funds.

## 13.6 Atomic Transfers

Internal transfers must debit and credit atomically.

## 13.7 Duplicate Posting

One event cannot post twice.

## 13.8 Reversals

Use compensating journals.

## 13.9 Projection Rebuild

Balances must be reconstructable from authoritative entries.

## 13.10 Cross-Asset Accounting

USDC, USDT and settlement-currency accounting must preserve per-asset balancing.

## 13.11 Reconciliation

Customer liabilities must reconcile with custody and settlement evidence.

## 13.12 Mandatory Invariants

1. Journals balance by asset.
2. Posted entries are immutable.
3. Available balances cannot be overspent.
4. Events post at most once.
5. Projections match authoritative postings.
6. Reversals preserve history.
7. Reservations cannot be consumed twice.
8. External financial operations remain traceable.

Any violation blocks engineering acceptance.

---

# 14. CUSTOMER REGISTRATION TESTING

Verify:

- Successful registration.
- Duplicate email protection.
- Duplicate username protection.
- Valid E.164 phone numbers.
- Weak password rejection.
- Consistent account creation.
- Email verification intent creation.
- Unverified-account restrictions.
- Idempotent email confirmation.

Phone ownership verification remains deferred after MVP.

Registration must not independently establish an authenticated session or KYC approval.

Wallet provisioning follows the approved wallet lifecycle.

---

# 15. AUTHENTICATION AND SESSION TESTING

## 15.1 Coverage

- Login.
- Invalid credentials.
- Password recovery.
- Email verification.
- Session creation.
- Session expiration.
- Refresh.
- Rotation.
- Logout.
- Revocation.
- Protected API access.
- Username availability.
- Rate limiting.
- Account restrictions.

## 15.2 Security

Verify:

- Cross-customer access is rejected.
- Revoked sessions cannot be reused.
- Password recovery invalidates affected sessions.
- Reset challenges expire and are single-use.
- Account-enumeration protections remain intact.
- Refresh tokens resist replay.
- Missing rate-limit policies fail safely.
- Sensitive responses are not improperly cached.

## 15.3 Financial Authorization

A valid login session does not automatically authorize a financial transaction.

Transaction-specific authorization and applicable eligibility controls must be enforced.

## 15.4 CI

Run authentication regression tests in GitHub Actions.

Real email delivery and hosted HTTPS behavior require separate staging acceptance.

## 15.5 Final Authentication Milestone

After Claude's authentication PR is reviewed and merged:

1. Run reproducible automated tests through GitHub Actions.
2. Verify the exact merged commit.
3. Complete necessary environment-specific testing in the main DEYPAY directory.
4. Record any remaining staging evidence.
5. Complete engineering acceptance before frontend implementation.

---

# 16. KYC AND COMPLIANCE TESTING

Verify:

- Not started.
- Draft.
- Submitted.
- Pending.
- Manual review.
- Approved.
- Rejected.
- Expired.

Test restrictions, provider responses, authorized review, audits and resubmission.

KYC verification implementation follows the approved deferred schedule.

Existing financial restrictions must not be bypassed.

---

# 17. WALLET MANAGEMENT TESTING

Verify:

- Wallet provisioning.
- Customer ownership.
- USDC and USDT support.
- Asset/network compatibility.
- Wallet status.
- Balance accuracy.
- Provisioning failure.
- Unsupported-network rejection.
- Deposit address ownership.

The initial operational blockchain is Solana.

Additional networks require separate implementation acceptance.

---

# 18. STABLECOIN DEPOSIT TESTING

## 18.1 Successful Deposits

Confirmed eligible deposits credit the correct customer once.

## 18.2 Pending Deposits

Unconfirmed deposits are not prematurely spendable.

## 18.3 Duplicate Events

Duplicate events cannot produce duplicate credits.

## 18.4 Unsupported Assets

Unsupported networks and tokens must not receive ordinary credits.

## 18.5 Reorganization

Test invalidated deposits and recovery behavior.

## 18.6 Reconciliation

Credits must reconcile with blockchain and custody evidence.

## 18.7 Auto-Settlement

Test:

- Dedicated address ownership.
- Linked-bank mapping.
- Correct Solana USDC detection.
- Required confirmation.
- Single conversion and payout.
- Duplicate webhook protection.
- Rates and fees.
- Failed conversion.
- Failed payout.
- Small-value deposits.
- Reconciliation.

Auto-Settlement shares the approved Off-Ramp Engine.

---

# 19. INTERNAL TRANSFER TESTING

Verify:

- Successful transfers.
- Insufficient funds.
- Self-transfer restrictions.
- Invalid recipients.
- Restricted recipients.
- Concurrency safety.
- Idempotency.
- Accurate history.
- Customer privacy.

Example:

Customer A sends 25 USDT to Customer B.

Expected:

- Sender decreases by 25 USDT.
- Recipient increases by 25 USDT.
- Aggregate customer liability remains unchanged.
- One balanced journal is posted.

---

# 20. EXTERNAL WITHDRAWAL TESTING

Verify:

1. Quote validation.
2. Authorization.
3. Balance availability.
4. Reservation.
5. Provider submission.
6. Status tracking.
7. Settlement confirmation.
8. Final accounting.

Additionally test invalid destinations, unsupported networks, insufficient funds, rejection, timeout, duplication, fees and reconciliation.

Unknown provider outcomes must not cause blind resubmission.

---

# 21. STABLECOIN CONVERSION TESTING

Verify:

- Supported conversion pairs.
- Quotes.
- Expiration.
- Quote tampering.
- Insufficient funds.
- Provider failure.
- Partial settlement.
- Fees and rounding.
- Cross-asset accounting.
- Recovery.

DEYPAY does not maintain a general-purpose NGN customer wallet.

NGN conversion occurs within approved funding, withdrawal or bank-payment workflows.

---

# 22. NGN ON-RAMP TESTING

Verify:

- Funding orders.
- Quotes.
- NGN payment instructions.
- Confirmed collection.
- Stablecoin delivery.
- Customer credit.
- Duplicate events.
- Incorrect funding amounts.
- Provider timeouts.
- Reconciliation.
- Eligibility restrictions.

NGN funding must not create a general-purpose customer NGN stored-value balance.

---

# 23. NGN OFF-RAMP TESTING

## 23.1 Successful Off-Ramp

Verify:

1. Stablecoin reservation.
2. Quote acceptance.
3. Nuvion conversion.
4. Nigerian bank payout.
5. Settlement verification.
6. Final accounting.

## 23.2 Invalid Beneficiary

Reject invalid bank details.

## 23.3 Bank Rejection

Failed payouts enter recovery.

## 23.4 Conversion Succeeds, Payout Fails

Do not restore the original stablecoin amount without actual refund or reconversion.

## 23.5 Unknown Outcome

Protect funds until reconciliation establishes the outcome.

## 23.6 Duplicate Payout Prevention

One instruction cannot produce duplicate payouts.

## 23.7 Minimum Transaction Amount

DEYPAY targets transactions starting at $1, subject to provider capability and route economics.

Test minimums, fees, rounding and unsupported amounts.

## 23.8 Automatic Off-Ramp

The shared engine supports:

- Manual bank withdrawal.
- Confirmed-deposit-triggered Auto-Settlement.

Both use common accounting, execution, reconciliation and recovery mechanisms.

---

# 24. PAY ANYONE TESTING

Verify:

- Customer eligibility.
- Bank beneficiary.
- Account verification.
- Recipient confirmation.
- Quote.
- Authorization.
- Reservation.
- Conversion.
- NGN payout.
- Settlement.
- History.
- Receipt.

Test invalid accounts, beneficiary substitution, expired quotes, insufficient funds, timeout, duplicate payment and payout failure.

A provider timeout must not cause duplicate execution.

---

# 25. BENEFICIARY AND BANK VERIFICATION TESTING

Verify:

- Valid accounts.
- Invalid account numbers.
- Unsupported banks.
- Bank-directory failure.
- Resolution timeout.
- Name mismatch.
- Duplicate beneficiaries.
- Ownership controls.
- Unauthorized modification.
- Beneficiary deletion.
- Private labels.
- Step-up authorization.

Renaming a saved beneficiary must not silently change its destination.

---

# 26. TRANSACTION HISTORY AND RECEIPT TESTING

Verify:

- Accurate amounts.
- Correct statuses.
- Fees.
- Pagination.
- Filtering.
- Privacy.
- Receipts.
- Pending transactions.
- Reversal history.

Transactions must not appear completed before settlement is verified.

---

# 27. PROVIDER INTEGRATION TESTING

## 27.1 Nuvion

Verify authorized capabilities:

- Authentication.
- Wallet provisioning.
- Deposit addresses.
- Deposit events.
- Stablecoin withdrawals.
- FX quotes.
- Conversion.
- Nigerian bank payouts.
- Status lookup.
- Webhooks.
- Reconciliation.

## 27.2 Capability Verification

A related endpoint does not automatically prove a complete supported financial workflow.

## 27.3 Sandbox

Use official Nuvion sandbox resources when available.

## 27.4 Controlled Simulation

Simulate failures deterministically without substituting simulations for real integration acceptance.

## 27.5 Provider Independence

The DEYPAY ledger must remain independent of Nuvion.

## 27.6 CI Security

Ordinary PR workflows must not initiate live financial transfers.

---

# 28. WEBHOOK TESTING

Test:

1. Valid signed webhook.
2. Invalid signature.
3. Missing signature.
4. Malformed payload.
5. Duplicate event.
6. Out-of-order event.
7. Unknown reference.
8. Wrong customer.
9. Replayed event.
10. Database failure.
11. Worker crash.
12. Provider-status disagreement.

Webhook receipt alone must not establish verified settlement.

---

# 29. IDEMPOTENCY AND CONCURRENCY TESTING

## 29.1 Duplicate Requests

Repeated requests create one financial effect.

## 29.2 Conflicting Keys

A reused idempotency key with changed request content is rejected.

## 29.3 Concurrent Spending

Concurrent requests cannot overspend balances.

## 29.4 Reservations

Reservations cannot consume the same funds twice.

## 29.5 Worker Retries

Retries cannot duplicate external execution.

## 29.6 Crash-Recovery Fixtures

Use zero-fee fixtures except where explicitly specified.

| Scenario | Required result |
|---|---|
| Same 10 USDC deposit observed multiple times | One credit journal |
| Two simultaneous 60 USDT withdrawals against 100 USDT | One reservation; available 40 |
| Provider accepts withdrawal but worker crashes | One submission; reserved funds retained until verification |
| 20 USDT converted to NGN 30,000, bank payout fails | One payout submission; NGN 30,000 remains owed until resolved |
| Credited 10 USDC deposit is invalidated | Correct compensating journal or recovery case |

Test crashes before and after database commits.

Committed financial state must remain correct.

---

# 30. TRANSACTION STATE MACHINE TESTING

Verify:

- Valid transitions.
- Invalid transitions.
- Terminal-state protection.
- Unknown outcomes.
- Recovery states.
- Repeated events.
- Out-of-order events.

Submission acknowledgement is not completed settlement.

Public statuses must match authoritative records.

---

# 31. FRONTEND AND USER EXPERIENCE TESTING

## 31.1 Authentication

Verify registration, login, email confirmation and recovery.

## 31.2 Protected Routes

Verify:

- Unauthenticated access is rejected.
- Customers access only their own accounts.
- Logout revokes access.
- Browser Back cannot reveal protected data after logout.
- Expired sessions lose access.

## 31.3 Profile

Display actual customer information from real backend APIs.

## 31.4 Financial Interfaces

Verify balances, fees and statuses when features are implemented.

## 31.5 Responsiveness

Test mobile and desktop independently.

## 31.6 Accessibility

Verify keyboard access, labels and meaningful errors.

## 31.7 Duplicate Submission

Repeated clicks must not create duplicate operations.

## 31.8 Design Rules

Follow:

- `docs/frontend/01-UI-DESIGN-RULES.md`
- `docs/frontend/02-FRONTEND-ENGINEERING-RULES.md`

---

# 32. SECURITY TESTING

## 32.1 Authentication

Test credential stuffing resistance, expiration, revocation, recovery abuse and rate limiting.

## 32.2 Authorization

Test cross-customer access, privilege escalation and protected resources.

## 32.3 API Security

Test invalid inputs, injection, excessive request sizes and sensitive error leakage.

## 32.4 Financial Security

Test unauthorized withdrawals, beneficiary substitution, quote manipulation and duplicate execution.

## 32.5 Provider Security

Test webhook signatures, provider authorization and credential protection.

## 32.6 Independent Assessment

Independent security assessment remains required before production financial launch.

---

# 33. FRAUD AND ABUSE TESTING

Test:

- Repeated withdrawals.
- Multiple related accounts.
- Suspicious transfers.
- Repeated failed logins.
- New-device transactions.
- Beneficiary changes.
- Velocity limits.
- Refund abuse.
- Suspicious conversions.

Expected behavior follows approved risk policies.

---

# 34. PERFORMANCE AND LOAD TESTING

## 34.1 API

Measure latency, throughput and errors.

## 34.2 Ledger

Measure posting latency, reservation latency and lock contention.

## 34.3 Workers

Measure throughput, queue depth, retries and recovery.

## 34.4 Load Profiles

Test normal, peak, sustained and burst traffic.

## 34.5 Remote Execution

Heavy load testing should run on approved remote infrastructure.

Avoid unnecessary stress testing on the owner's local computer.

Do not perform uncontrolled production load testing.

---

# 35. RELIABILITY AND RESILIENCE TESTING

Verify:

- Database outages.
- Provider outages.
- Worker restarts.
- Network timeouts.
- Duplicate callbacks.
- Partial failures.
- Recovery.

Retries must not duplicate financial movements.

---

# 36. DISASTER RECOVERY TESTING

## 36.1 Backup

Verify database backups.

## 36.2 Restore

Restore into an isolated environment.

## 36.3 Ledger Integrity

Restored records must preserve ledger invariants.

## 36.4 Reconciliation

Recover an externally settled operation without resubmitting the original payment.

## 36.5 Recovery Objectives

Measure against approved thresholds.

## 36.6 Execution

Use approved isolated infrastructure for expensive recovery tests.

---

# 37. ACCESSIBILITY AND COMPATIBILITY TESTING

Verify:

- Supported browsers.
- Android and iOS devices.
- Desktop responsiveness.
- Keyboard navigation.
- Screen readers.
- Low-connectivity handling.
- Error messages.
- Reduced motion.

Target WCAG 2.2 AA where applicable.

Run automated accessibility checks in CI where practical.

---

# 38. REGULATORY AND COMPLIANCE VALIDATION

Verify implemented requirements covering:

- KYC restrictions.
- AML/CFT controls.
- Customer protection.
- Data protection.
- Provider permissions.
- Financial eligibility.
- Legal and operational readiness.

Compliance work follows the approved development schedule.

Internal testing does not replace provider approval or legal authorization for live-money operations.

---

# 39. END-TO-END TESTING

| ID | Scenario | Expected outcome |
|---|---|---|
| E2E-001 | Register and verify customer | Account created correctly |
| E2E-002 | Complete approved KYC | Eligible financial access |
| E2E-003 | Reject unapproved KYC transaction | Transaction blocked |
| E2E-004 | Provision USDC wallet | Correct address |
| E2E-005 | Confirmed deposit | Single credit |
| E2E-006 | Duplicate webhook | No duplicate credit |
| E2E-007 | Internal USDT transfer | Atomic transfer |
| E2E-008 | Overspending attempt | Rejected |
| E2E-009 | Concurrent transfers | No overspending |
| E2E-010 | Stablecoin withdrawal | Correct settlement |
| E2E-011 | Withdrawal timeout | No duplicate execution |
| E2E-012 | USDC-to-USDT conversion | Correct accounting |
| E2E-013 | Expired quote | Rejected |
| E2E-014 | NGN on-ramp | Correct stablecoin credit |
| E2E-015 | NGN off-ramp | Correct bank payout |
| E2E-016 | Pay Anyone | Settled and reconciled |
| E2E-017 | Failed bank payout | Correct outstanding obligation |
| E2E-018 | Unauthorized access | Denied |
| E2E-019 | Restore and reconcile | No duplicate execution |
| E2E-020 | Reverse transaction | Compensating journal |
| E2E-021 | QR code flows | Correct destination and authorization |
| E2E-022 | Payment link resolution | Correct access and validity rules |
| E2E-023 | Transaction support case | Customer isolation and safe support access |
| E2E-024 | Activate Auto-Settlement | Correct bank/address mapping |
| E2E-025 | Solana USDC Auto-Settlement deposit | One conversion and payout |
| E2E-026 | Duplicate Auto-Settlement notification | No duplicate settlement |
| E2E-027 | Auto-Settlement bank failure | Recoverable financial obligation |
| E2E-028 | Deposit below payout threshold | Funds correctly tracked without unsafe execution |

Applicable critical scenarios must pass before production enablement.

---

# 40. REGRESSION TESTING

## 40.1 Objective

Prevent new functionality from breaking accepted behavior.

## 40.2 Scope

Include authentication, KYC, wallets, ledger, deposits, withdrawals, transfers, conversion, NGN payments, Nuvion, Auto-Settlement and transaction history.

## 40.3 CI

Critical regression suites run in GitHub Actions.

## 40.4 Release

The applicable full regression suite must pass before production financial releases.

---

# 41. AUTOMATED TESTING AND CI/CD

**Mandatory for all DEYPAY engineering agents.**

## 41.1 CI Platform

Primary platform: GitHub Actions.

Repository:

`Kenule-Africa/Daypay`

Existing workflow:

`.github/workflows/phase-one.yml`

Reuse and extend existing CI infrastructure. Do not create a duplicate testing system unnecessarily.

## 41.2 Testing Workflow

    IMPLEMENT CODE
         |
    COMMIT CHANGES
         |
    PUSH TO GITHUB
         |
    GITHUB ACTIONS RUNS
         |
    AGENT CHECKS RESULT
         |
      PASS / FAIL
         |
    FIX FAILURES
         |
    PUSH CORRECTION
         |
    VERIFY FINAL COMMIT
         |
    REPORT EVIDENCE

## 41.3 Backend CI

Run from `backend/`:

    cargo fmt --all -- --check
    cargo clippy --locked --workspace --all-targets -- -D warnings
    cargo build --locked --workspace
    cargo test --locked --workspace

Also run applicable PostgreSQL and HTTP integration tests.

## 41.4 PostgreSQL CI

GitHub Actions must:

1. Provision isolated PostgreSQL.
2. Generate temporary configuration.
3. Apply migrations.
4. Execute integration tests.
5. Verify permissions and constraints.
6. Execute applicable concurrency tests.
7. Clean up resources.

## 41.5 Frontend CI

Run from `frontend/`:

    npm ci
    npm run lint
    npm run typecheck
    npm test
    npm run build

## 41.6 Mandatory Rules

- Test the exact pushed commit.
- Failed required checks block acceptance.
- Required jobs cannot be silently skipped.
- Do not expose production credentials.
- Do not connect CI to production databases.
- Do not execute live financial transfers in ordinary CI.
- Do not remove tests just to obtain a passing result.

## 41.7 Workflow Optimization

Use:

- Parallel jobs.
- Safe Cargo caching.
- npm dependency caching.
- Reduced redundant downloads.
- Appropriate runners.
- Targeted checks during development.

Keep mandatory release-level verification intact.

## 41.8 Agent Responsibility

The implementing agent is responsible for monitoring and resolving CI failures attributable to its changes.

---

# 42. DEFECT MANAGEMENT

## 42.1 Critical

Loss of funds, double spending, unbalanced journals, unauthorized withdrawal or duplicate payouts.

## 42.2 High

Major financial failure, sensitive-data exposure or serious authorization defects.

## 42.3 Medium

Recoverable functional failure.

## 42.4 Low

Minor visual or cosmetic defects.

## 42.5 Lifecycle

    NEW
     |
    TRIAGED
     |
    ASSIGNED
     |
    IN PROGRESS
     |
    READY FOR RETEST
     |
    VERIFIED
     |
    CLOSED

## 42.6 Release Blocking

Critical defects block release.

High-risk security and financial defects require resolution or formal risk handling.

---

# 43. TEST COVERAGE AND TRACEABILITY

Critical requirements must map to test cases.

Record:

- Requirement ID.
- Test ID.
- Description.
- Test level.
- Priority.
- Automation status.
- Environment.
- Result.
- Defect.
- Commit SHA.
- GitHub Actions URL.

Ledger invariants and critical authorization boundaries require automated coverage.

A coverage percentage alone does not establish correctness.

---

# 44. TEST EXECUTION AND REPORTING

## 44.1 Mandatory Test Record

Every engineering acceptance report identifies:

| Field | Requirement |
|---|---|
| Repository | Exact repository |
| Branch | Tested branch |
| Commit SHA | Exact tested revision |
| Workflow | GitHub Actions workflow |
| Run URL | Direct evidence link |
| Status | Completed or otherwise |
| Required jobs | Pass/fail conclusions |
| Failed tests | Explicitly identified |
| Skipped tests | Explicitly identified |
| Outstanding dependencies | Recorded |
| Acceptance recommendation | Supported by evidence |

## 44.2 Verification

Agents must not:

- Assume CI succeeded.
- Report queued jobs as passing.
- Reuse old passing commits.
- Ignore failed jobs.
- Invent test counts.
- Claim staging acceptance based only on CI.

## 44.3 Failure Handling

1. Identify failed job.
2. Inspect logs.
3. Determine cause.
4. Fix defect.
5. Push correction.
6. Verify the new run.

## 44.4 Evidence Retention

Retain relevant CI results without leaking secrets.

---

# 45. RELEASE ENTRY AND EXIT CRITERIA

## 45.1 Entry

A release candidate may enter testing when:

1. Implementation is complete.
2. Reviews are available.
3. Migrations are included.
4. Tests exist.
5. Dependencies are available.
6. Limitations are recorded.

## 45.2 Exit

Engineering acceptance requires:

1. Required CI passes.
2. Results match the accepted commit.
3. Financial invariants pass.
4. Security checks pass.
5. Integration tests pass.
6. No unresolved critical defects.
7. Environment-specific gaps are recorded.
8. Owner acceptance is documented.

## 45.3 Distinction

Passing CI does not establish production approval, real email delivery, Nuvion readiness or legal authorization.

---

# 46. PRODUCTION READINESS AND SIGN-OFF

## 46.1 Internal Strategy Approval

The DEYPAY Project Owner has approved this testing strategy across all listed internal governance roles.

This is authorization to adopt and implement the strategy.

## 46.2 Actual Release Readiness

Production financial launch additionally requires:

- Confirmed custody arrangements.
- Verified provider capabilities.
- Applicable external authorization.
- Security assessment evidence.
- Monitoring.
- Reconciliation.
- Incident response.
- Financial recovery procedures.

Document approval must not be mistaken for completed technical testing or external certification.

---

# 47. POST-DEPLOYMENT VALIDATION

## 47.1 Smoke Tests

Verify API availability, authentication, database connections, workers and approved provider connections.

## 47.2 Monitoring

Monitor ledger imbalances, duplicate events, failed payouts, pending transactions and reconciliation differences.

## 47.3 Rollback

Application rollback must not reverse completed external financial transactions.

## 47.4 Incidents

Critical defects require incident-response procedures.

---

# 48. TEST RISKS AND MITIGATIONS

| Risk | Mitigation |
|---|---|
| Slow local computer | GitHub Actions for heavy tests |
| Repeated compilation | Safe CI caching and focused local checks |
| Old CI results | Exact commit verification |
| Missing CI access | Explicitly report inability to verify |
| Broken workflow | Repair CI; don't bypass required checks |
| Incomplete provider sandbox | Separate simulation from real acceptance |
| Unverified Nuvion capability | End-to-end confirmation |
| Financial defects | Mandatory invariant tests |
| Race conditions | Concurrency tests |
| Duplicate payments | Idempotency and reconciliation |
| Secret leakage | Restricted CI access and logs |
| Missing security review | Independent assessment |
| Incomplete recovery | Fault injection |
| Rushed release | Evidence-based gates |

---

# 49. TESTING DELIVERABLES

1. Approved test strategy.
2. Requirement traceability records.
3. Automated unit tests.
4. PostgreSQL integration tests.
5. API contract tests.
6. Financial invariant tests.
7. Provider integration tests.
8. End-to-end tests.
9. Security evidence.
10. Performance results.
11. Recovery results.
12. Defect records.
13. GitHub Actions evidence.
14. Engineering acceptance records.
15. Production readiness evidence.

These are testing and execution artifacts, not additional mandatory top-level documents.

---

# 50. ACCEPTANCE CRITERIA AND FINAL DEFINITION

The DEYPAY testing strategy is satisfied when:

1. Approved features have defined test coverage.
2. Ledger invariants have automated tests.
3. Internal transfers are atomic.
4. Duplicate execution is prevented.
5. Provider timeouts are tested.
6. Failed payouts are recoverable.
7. Restrictions are enforced.
8. Authentication and authorization are tested.
9. Sensitive data is protected.
10. Migrations are verified.
11. API contracts are validated.
12. End-to-end journeys are tested.
13. Performance is measurable.
14. Recovery procedures are tested.
15. GitHub CI provides reliable evidence.
16. Production release remains independently verified.

---

# 51. MANDATORY GITHUB ACTIONS EXECUTION POLICY

## 51.1 Default Rule

Claude, Codex and future engineering agents must execute expensive tests on GitHub Actions wherever feasible.

This includes:

- Full Rust builds.
- Full Rust test suites.
- PostgreSQL integration.
- API integration.
- Authentication regression.
- Financial invariants.
- Frontend builds.
- Broad static analysis.
- Long-running automated suites.

## 51.2 Local Exceptions

Local testing is permitted for:

- Targeted tests.
- Quick formatting checks.
- Debugging.
- Manual UI review.
- Private configuration checks.
- Local hardware dependencies.
- Authorized environment-specific acceptance.

Do not repeatedly execute complete local builds when the corresponding GitHub checks already cover the same revision.

## 51.3 Agent Ownership

Agents monitor CI triggered by their branches.

Code pushed does not mean testing completed.

## 51.4 Efficient CI

Avoid unrelated dependency upgrades, duplicate workflows and unnecessary full-suite reruns.

## 51.5 Resource Awareness

GitHub-hosted runners have plan-dependent limits and usage allowances.

Agents should use CI efficiently without weakening quality.

---

# 52. AGENT CI MONITORING PROTOCOL

## 52.1 Trigger

After implementation:

1. Commit changes.
2. Push to GitHub.
3. Find the workflow run.
4. Confirm the commit SHA.

## 52.2 Access

Agents may use:

- GitHub CLI.
- GitHub integrations.
- GitHub Actions REST API.
- Authorized GitHub browser access.

Use whatever mechanism is available.

## 52.3 Required Verification

Verify:

- Correct repository.
- Correct commit SHA.
- Completed workflow.
- Successful required jobs.
- No silently skipped mandatory tests.

## 52.4 Failed Jobs

1. Retrieve logs.
2. Identify failure.
3. Fix the implementation.
4. Push correction.
5. Verify updated CI.

## 52.5 Missing Access

If an agent cannot retrieve CI status or logs, report the limitation.

Lack of access is not evidence of success.

## 52.6 Agent Completion Report

    DEYPAY ENGINEERING VERIFICATION

    Task:
    Branch:
    Commit SHA:
    Pull Request:
    GitHub Actions URL:

    Backend CI:
    Frontend CI:
    PostgreSQL Integration:
    Relevant Regression Tests:

    Failed Tests:
    Skipped Tests:
    Outstanding Dependencies:

    Implementation Status:
    Recommended Next Action:

Only report tests that actually ran.

## 52.7 Merge Ownership

CI success does not authorize an engineering agent to merge its own PR.

The repository owner retains merge approval.

---

# 53. LOCAL TESTING AND ENVIRONMENT-SPECIFIC ACCEPTANCE

## 53.1 Main DEYPAY Directory

The primary local application directory is:

`~/Desktop/Projects/DeyPay`

It contains the owner's existing private `.env` and local testing tools.

## 53.2 Final Authentication Testing

After Claude's authentication implementation is reviewed and merged:

1. GitHub Actions runs reproducible automated tests.
2. The main DEYPAY directory is updated.
3. Claude performs the remaining environment-specific checks.
4. Claude reports real results and gaps.
5. The owner authorizes the next milestone.

Do not rerun the entire Rust workspace locally without a specific need when CI has verified the same commit.

## 53.3 Private Configuration

Never:

- Commit the `.env`.
- Upload private database credentials.
- Print secrets.
- Copy sensitive configuration into CI artifacts.
- Give unnecessary access to production resources.

## 53.4 Email Verification

Real Resend acceptance requires an authorized configured environment.

CI can validate email-generation and application logic, but cannot claim actual delivery without a successful external delivery test.

## 53.5 Provider Testing

Actual Nuvion tests require approved credentials and environments.

Money-moving tests need separate authorization and must not run automatically on ordinary pull requests.

---

# 54. CI SECURITY, PERFORMANCE AND MAINTENANCE RULES

## 54.1 Least Privilege

Workflows use minimum required permissions.

## 54.2 Dependency Security

Use compatible stable dependencies and reviewed action revisions.

## 54.3 Caching

Cache dependencies and compilation outputs when beneficial, with appropriate integrity and security controls.

## 54.4 Parallel Jobs

Run independent jobs concurrently when beneficial.

## 54.5 Required Checks

Mandatory CI gates must not be weakened to improve runtime.

## 54.6 Infrastructure Failures

Distinguish runner/network failures from application defects.

Infrastructure failures may justify controlled reruns.

## 54.7 Isolation

CI must not interfere with production, other agents or unrelated branches.

## 54.8 Evidence

Retain appropriate logs and test reports without secrets.

## 54.9 Improvements

Allow future CI improvements, including:

- Better caching.
- Targeted developer feedback.
- Reusable workflows.
- Parallel test suites.
- Dedicated performance runners.

Every optimization must preserve required acceptance coverage.

---

# FINAL TESTING PRINCIPLE

DEYPAY must not rely on a successful interface demonstration or passing build alone as proof of financial correctness.

A successful transaction must be verified at the application, ledger and external settlement levels.

A failed transaction must preserve accurate financial obligations.

Duplicate requests must not create duplicate money movement.

Uncertain provider responses must not trigger unsafe retries.

Database or worker failure must not compromise financial integrity.

Unauthorized actors must not access another customer's funds or sensitive information.

**GitHub Actions is DEYPAY's primary automated testing platform.**

**Claude and Codex must verify the exact tested commit, inspect failed jobs and provide real CI evidence before claiming engineering completion.**

**The owner's computer must not carry the burden of repeated full automated builds and integration tests.**

The DEYPAY standard is financial value that remains correct, protected, traceable and recoverable under normal and abnormal conditions.

---

# DOCUMENT APPROVAL

| Role | Status |
|---|---|
| Product Owner | Approved — 26 November 2026 |
| Engineering Lead | Approved — 26 November 2026 |
| QA Lead | Approved — 26 November 2026 |
| Security Reviewer | Approved — 26 November 2026 |
| Financial Operations Lead | Approved — 26 November 2026 |
| Compliance Lead | Approved — 26 November 2026 |

**Approval Authority:** DEYPAY Project Owner  
**Document ID:** DP-TEST-007  
**Version:** 1.1  
**Status:** Approved Engineering Baseline  
**Approval Status:** Approved  
**Approval Date:** 26 November 2026

---

**END OF DOCUMENT**

**DEYPAY SOFTWARE TEST STRATEGY AND QUALITY ASSURANCE SPECIFICATION**