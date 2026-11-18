# DEYPAY (DP)
# SYSTEM ARCHITECTURE DOCUMENT (SAD)
## Backend Architecture, Financial Infrastructure, System Components and Engineering Design

**Document ID:** DP-ARCH-002  
**Version:** 1.0  
**Status:** Proposed Architecture Baseline – Pending Engineering and Compliance Review  
**Product:** DEYPAY  
**Initial Market:** Nigeria  
**Initial Application:** Progressive Web Application (PWA)  
**Backend Language:** Rust  
**Backend Framework:** Axum  
**Primary Database:** PostgreSQL  
**Architecture Pattern:** Modular Monolith  
**Preferred Financial Infrastructure:** Nuvion  
**Document Classification:** Internal – Engineering and Product  
**Date:** 17 November 2026  
**Related Document:** DEYPAY Functional Requirements Specification (FRS), Version 1.0

---

# DOCUMENT CONTROL

## Purpose

This document defines the proposed technical architecture of DEYPAY.

It establishes the structural and operational foundation for implementing DEYPAY's financial backend, integrating third-party infrastructure, maintaining customer financial records, processing payments and supporting future product expansion.

This document is intended to guide:

- Backend engineers.
- Frontend engineers.
- Software architects.
- DevOps engineers.
- Security engineers.
- Database engineers.
- Quality assurance engineers.
- Financial operations teams.
- Compliance stakeholders.
- Technical partners.
- AI-assisted development environments.

The architecture described here is the implementation baseline for the DEYPAY MVP.

Changes to fundamental architectural decisions must be documented through Architecture Decision Records (ADRs).

## Authority and Precedence

The FRS defines the approved product requirements.

This SAD defines the proposed technical implementation.

If a technical design conflicts with an approved product requirement, the conflict must be resolved before implementation.

If a product requirement conflicts with applicable law, regulatory approval or financial safeguarding obligations, the relevant feature must not be activated until the conflict is resolved.

Provider documentation and API behavior must be validated against the production contract before deployment.

---

# TABLE OF CONTENTS

1. Executive Architecture Summary
2. Architectural Goals
3. Confirmed Product Decisions
4. Architecture Principles
5. System Context
6. High-Level Architecture
7. Technology Stack
8. Rust Modular Monolith Design
9. Backend Module Boundaries
10. Domain Modeling
11. Database Architecture
12. Financial Ledger Architecture
13. Wallet Architecture
14. Nuvion Integration Architecture
15. Provider Abstraction Layer
16. Deposit Architecture
17. Withdrawal Architecture
18. Internal Transfer Architecture
19. Pay Anyone Architecture
20. Conversion and Liquidity Architecture
21. Transaction Orchestration
22. Event-Driven Processing
23. Webhook Architecture
24. Authentication and Authorization
25. KYC and Compliance Architecture
26. Security Architecture
27. API Architecture
28. Frontend and PWA Architecture
29. Administration Architecture
30. Notifications Architecture
31. Reconciliation Architecture
32. Observability Architecture
33. Infrastructure and Deployment
34. Configuration and Secrets
35. Reliability and Failure Recovery
36. Performance and Scalability
37. Testing Architecture
38. Development Environments
39. Repository Structure
40. Coding and Engineering Standards
41. Architecture Decision Records
42. Technical Risks
43. Regulatory Dependencies
44. Implementation Sequence
45. Architecture Acceptance Criteria
46. Open Technical Decisions
47. Final Architecture Definition

---

# 1. EXECUTIVE ARCHITECTURE SUMMARY

DEYPAY will be developed as an independent, stablecoin-first financial technology platform.

The backend will be implemented primarily in Rust using the Axum web framework, Tokio asynchronous runtime and PostgreSQL relational database.

The system will initially follow a modular monolith architecture.

A modular monolith provides one deployable backend application with logically separated business modules.

This architecture is selected because it provides:

- Strong transactional consistency.
- Lower initial infrastructure complexity.
- Easier debugging.
- Clear domain boundaries.
- Efficient development.
- Reduced operational overhead.
- A foundation for future service extraction.

DEYPAY will maintain independent control over:

- Customer identities.
- Authentication.
- Customer profiles.
- Wallet profiles.
- Customer financial balances.
- Double-entry accounting.
- Internal transfers.
- Payment orchestration.
- Transaction history.
- Business rules.
- Fees.
- Administrative operations.
- Compliance workflows.
- Reconciliation.

Nuvion will be integrated as an external financial infrastructure provider through a dedicated adapter.

The integration must not make Nuvion the owner of DEYPAY's internal application architecture.

Other providers may be introduced for Nigerian bank payments, identity verification, custody, blockchain monitoring and related services.

The customer-facing application will communicate exclusively with DEYPAY-controlled APIs.

Financial operations will be implemented using explicit transaction state machines, idempotency, balance reservations, reliable event processing and reconciliation.

The financial ledger will be the authoritative record of customer financial entitlements and internal accounting.

External provider records will be independently reconciled against the ledger.

---

# 2. ARCHITECTURAL GOALS

## 2.1 Primary Goals

The architecture must:

1. Support the complete approved DEYPAY MVP.
2. Protect customer financial records.
3. Prevent duplicate financial execution.
4. Prevent unauthorized balance changes.
5. Support USDC and USDT.
6. Support internal customer transfers.
7. Support stablecoin deposits and withdrawals.
8. Support fiat conversion.
9. Support Nigerian bank payouts through approved providers.
10. Maintain accurate transaction histories.
11. Provide operational visibility.
12. Support regulatory controls.
13. Remain independent of individual financial infrastructure providers.
14. Support future product expansion.

## 2.2 Engineering Goals

The architecture should prioritize:

- Correctness before optimization.
- Security before convenience.
- Financial consistency before throughput.
- Simplicity before distributed complexity.
- Explicit interfaces.
- Testability.
- Maintainability.
- Observability.
- Recoverability.

## 2.3 Non-Goals

The MVP architecture will not initially require:

- A microservices ecosystem.
- Kubernetes.
- Multiple independent financial databases.
- Blockchain smart contract development.
- A proprietary blockchain.
- A proprietary custody engine.
- A proprietary cryptocurrency exchange.
- An internal matching engine.
- High-frequency trading infrastructure.
- Complex distributed consensus.

These technologies must not be introduced without a demonstrated requirement.

---

# 3. CONFIRMED PRODUCT DECISIONS

The architecture shall preserve the following decisions.

**AD-001:** DEYPAY is a stablecoin-first platform.

**AD-002:** USDC and USDT are the initial customer wallet assets.

**AD-003:** No general-purpose NGN stored-value customer wallet is included in the initial Model B design.

**AD-004:** Users may register before completing KYC.

**AD-005:** Financial operations require the appropriate KYC and eligibility approval.

**AD-006:** Eligible customers automatically receive USDC and USDT wallet profiles.

**AD-007:** The dashboard is wallet-first.

**AD-008:** Users can transfer internally through usernames and supported phone-number discovery.

**AD-009:** Users can pay third-party Nigerian bank accounts using stablecoin balances.

**AD-010:** DEYPAY owns its internal financial ledger.

**AD-011:** Nuvion is a preferred infrastructure candidate, not the application owner.

**AD-012:** External providers are accessed through replaceable adapters.

**AD-013:** The initial application is a PWA.

**AD-014:** The backend is built and tested before primary frontend development.

**AD-015:** Rust is the backend language.

**AD-016:** The backend initially uses a modular monolith.

---

# 4. ARCHITECTURE PRINCIPLES

## 4.1 Domain Ownership

Every business domain must own its internal rules and operations.

For example:

- Identity owns authentication.
- Wallets owns wallet profiles.
- Ledger owns accounting.
- Payments owns payment orchestration.
- Compliance owns financial eligibility rules.
- Integrations owns external provider communication.

Modules must not directly modify another module's private data.

## 4.2 Provider Independence

Provider-specific code must remain isolated.

Business services must depend on internal provider interfaces rather than concrete Nuvion implementations.

## 4.3 Financial Integrity

Every customer balance change must be supported by a valid ledger operation.

Financial records must not be modified directly by controllers, provider webhooks or administrative interfaces.

## 4.4 Idempotency

Every externally triggered financial operation must have a mechanism that prevents unintended duplicate execution.

## 4.5 Least Privilege

Every service, administrator, database account and external integration must receive only the permissions necessary to perform its function.

## 4.6 Auditability

Every material financial and administrative action must be traceable.

## 4.7 Fail-Safe Processing

An unknown transaction outcome must not automatically be treated as a failure.

## 4.8 Explicit Dependencies

Unverified external capabilities must be modeled as dependencies rather than assumed facts.

---

# 5. SYSTEM CONTEXT

DEYPAY consists of four major environments.

## 5.1 Customer Environment

Customers access DEYPAY through a responsive web application or installed PWA.

## 5.2 DEYPAY Application Environment

The application environment contains:

- Public API.
- Authentication.
- Customer management.
- Wallet management.
- Financial ledger.
- Payment processing.
- Provider orchestration.
- Compliance.
- Administration.
- Background processing.

## 5.3 External Financial Environment

External providers supply:

- Wallet infrastructure.
- Stablecoin custody.
- Blockchain transfers.
- Currency conversion.
- Nigerian bank payouts.
- Identity verification.
- Transaction screening.

## 5.4 Operational Environment

The operational environment contains:

- Monitoring.
- Logging.
- Alerts.
- Backups.
- Reconciliation.
- Support tools.
- Security controls.

---

# 6. HIGH-LEVEL ARCHITECTURE

The proposed architecture is:

```text
                 DEYPAY CUSTOMERS
                        |
                        v
                DEYPAY WEB / PWA
                        |
                        v
                HTTPS / API EDGE
                        |
                        v
              DEYPAY RUST BACKEND
              ====================
              Authentication
              Customer Profiles
              KYC
              Wallets
              Ledger
              Deposits
              Withdrawals
              Transfers
              Payments
              FX
              Compliance
              Notifications
              Administration
              Reconciliation
              ====================
                   |       |
                   |       +----------------+
                   v                        v
             POSTGRESQL                REDIS / CACHE
                   |
                   v
          DURABLE EVENT / JOB SYSTEM
                   |
                   v
          PROVIDER INTEGRATION LAYER
                   |
        +----------+-----------+
        |          |           |
        v          v           v
      NUVION    NGN PAYMENT   KYC / AML
                PROVIDER      PROVIDERS
        |
        v
   CUSTODY / BLOCKCHAIN
   INFRASTRUCTURE
```

All customer financial requests must pass through DEYPAY's backend.

The frontend must not possess privileged financial provider credentials.

---

# 7. TECHNOLOGY STACK

## 7.1 Backend

Language: Rust.

Framework: Axum.

Asynchronous runtime: Tokio.

HTTP client: Reqwest.

Serialization: Serde.

Database access: SQLx.

API specification: OpenAPI.

Tracing: Tracing and OpenTelemetry-compatible tooling.

Error handling: Typed application errors.

## 7.2 Database

Primary database: PostgreSQL.

PostgreSQL will store:

- Customer records.
- Authentication records.
- Wallet metadata.
- Financial ledger.
- Transactions.
- Provider references.
- Compliance records.
- Audit records.
- Reconciliation data.

## 7.3 Cache and Ephemeral Data

Redis may be used for:

- Rate limiting.
- Temporary non-financial caching.
- Short-lived authentication challenges.
- Selected distributed coordination functions.

Redis must not be the authoritative financial ledger.

## 7.4 Background Processing

The MVP should initially use PostgreSQL-backed durable jobs and a transactional outbox.

This keeps the infrastructure simple while supporting reliable asynchronous operations.

A dedicated message broker may be introduced later if operational requirements justify it.

## 7.5 Frontend

Recommended:

- TypeScript.
- React.
- Next.js.
- Responsive design system.
- PWA support.
- Typed API client.

The frontend framework choice remains subject to a short engineering validation.

## 7.6 Infrastructure

- Docker.
- Managed PostgreSQL.
- Managed application hosting.
- Object storage.
- Secrets management.
- Monitoring.
- Automated backups.
- CI/CD.

The exact cloud provider is not yet selected.

---

# 8. RUST MODULAR MONOLITH DESIGN

## 8.1 Design Approach

DEYPAY should be implemented as a Rust workspace containing logically separated crates or modules.

The backend will initially produce:

1. One API application.
2. One background worker application.

Both applications may share domain and infrastructure libraries.

The worker is a separate process, not an independently owned financial microservice.

## 8.2 Architectural Layers

The backend will use four conceptual layers.

### Domain Layer

Contains business rules and domain entities.

Examples:

- Wallet.
- Money.
- LedgerAccount.
- Payment.
- Transfer.
- TransactionStatus.

The domain layer should avoid dependencies on HTTP frameworks and concrete providers.

### Application Layer

Contains use cases and workflow coordination.

Examples:

- RegisterCustomer.
- ProvisionWallets.
- InitiateWithdrawal.
- TransferInternally.
- ExecuteBankPayment.
- ReconcileProviderTransactions.

### Infrastructure Layer

Contains technical implementations.

Examples:

- PostgreSQL repositories.
- Nuvion API adapter.
- Nigerian payout adapter.
- Email provider.
- KYC provider.
- Event storage.

### Interface Layer

Contains external application interfaces.

Examples:

- HTTP routes.
- Request validation.
- Response serialization.
- Webhook endpoints.
- Administrative endpoints.

## 8.3 Dependency Direction

The intended dependency direction is:

```text
HTTP / WEBHOOK INTERFACES
           |
           v
    APPLICATION USE CASES
           |
           v
      DOMAIN RULES
           ^
           |
  INFRASTRUCTURE ADAPTERS
```

Infrastructure implementations must satisfy interfaces required by the application or domain.

Business logic must not depend directly on HTTP request structures or provider-specific response formats.

---

# 9. BACKEND MODULE BOUNDARIES

## 9.1 Identity Module

Responsibilities:

- Registration.
- Authentication.
- Password management.
- Session management.
- MFA.
- Device management.
- Account recovery.

Does not own:

- Wallet balances.
- Financial transfers.
- Ledger entries.

## 9.2 Customer Module

Responsibilities:

- Customer profile.
- Username.
- Verified contact details.
- Account preferences.
- Customer status.

## 9.3 KYC Module

Responsibilities:

- KYC submissions.
- Verification workflows.
- Provider verification references.
- Review status.
- Eligibility evidence.

## 9.4 Wallet Module

Responsibilities:

- Wallet profiles.
- Asset/network combinations.
- Deposit addresses.
- Wallet provisioning.
- Provider wallet references.

Does not directly change financial balances.

## 9.5 Ledger Module

Responsibilities:

- Ledger accounts.
- Journal entries.
- Posting.
- Reservations.
- Balance calculations.
- Financial invariants.

Only the ledger module may perform ledger postings.

## 9.6 Deposit Module

Responsibilities:

- Incoming deposit detection.
- Confirmation tracking.
- Deposit screening.
- Deposit credit workflow.

## 9.7 Withdrawal Module

Responsibilities:

- Withdrawal requests.
- Destination validation.
- Authorization.
- Reservation.
- Provider execution.
- Settlement tracking.

## 9.8 Payments Module

Responsibilities:

- Internal transfers.
- Bank payments.
- Beneficiaries.
- Payment requests.
- Payment state machines.

## 9.9 FX Module

Responsibilities:

- Currency quotes.
- Quote expiry.
- Conversion requests.
- Pricing calculations.
- Conversion execution.

## 9.10 Provider Integration Module

Responsibilities:

- External API communication.
- Provider authentication.
- Provider reference mapping.
- Provider errors.
- Provider event normalization.

## 9.11 Compliance Module

Responsibilities:

- Financial eligibility.
- Transaction limits.
- Risk screening.
- Account restrictions.
- Compliance investigations.

## 9.12 Reconciliation Module

Responsibilities:

- Provider record retrieval.
- Internal/external matching.
- Discrepancy detection.
- Reconciliation reports.

## 9.13 Administration Module

Responsibilities:

- Administrative access.
- Operational actions.
- Review workflows.
- Financial investigation interfaces.

## 9.14 Notification Module

Responsibilities:

- In-app notifications.
- Email.
- Supported push notifications.
- Notification preferences.

---

# 10. DOMAIN MODELING

The architecture must distinguish between different financial concepts.

## 10.1 Customer

Represents a DEYPAY account holder.

A customer has one stable internal identifier.

## 10.2 Wallet Profile

Represents a customer's access to a supported asset.

Example:

Customer A → USDC wallet profile.

Customer A → USDT wallet profile.

## 10.3 Blockchain Address

Represents an address on a particular blockchain network.

A wallet profile may be associated with multiple supported addresses.

## 10.4 Ledger Account

Represents an accounting position for a specific owner, asset and purpose.

## 10.5 Transaction

Represents a financial business operation.

Examples:

- Deposit.
- Withdrawal.
- Internal transfer.
- Conversion.
- Bank payment.

## 10.6 Payment Instruction

Represents a request to transfer value to a recipient.

## 10.7 Provider Operation

Represents a financial operation submitted to an external provider.

## 10.8 Money

Money must be modeled using:

- Asset identifier.
- Exact amount.
- Asset precision.

Floating-point arithmetic must not be used.

## 10.9 Asset Identity

Assets must be identified by explicit asset/network/token information.

For example, USDC on Ethereum and USDC on Base may share a customer-facing asset symbol while representing different blockchain token contracts.

The system must never assume that equal symbols imply identical transfer networks or custody positions.

---

# 11. DATABASE ARCHITECTURE

## 11.1 Primary Database

PostgreSQL will serve as the authoritative relational database.

## 11.2 Logical Schemas

Recommended logical separation:

- identity.
- customers.
- compliance.
- wallets.
- ledger.
- payments.
- fx.
- integrations.
- operations.
- audit.

Separate PostgreSQL schemas are optional, but domain ownership must remain clear.

## 11.3 Core Requirements

The database must support:

- ACID transactions.
- Foreign key constraints.
- Unique constraints.
- Check constraints.
- Transaction isolation.
- Row-level locking where necessary.
- Reliable migrations.
- Backup and recovery.

## 11.4 Financial Transactions

Financial database operations must execute atomically.

Example:

An internal transfer must not debit the sender without crediting the recipient.

The complete financial posting must succeed or fail as one database transaction.

## 11.5 Migration Strategy

All schema changes must be version-controlled.

Production migrations must be reviewed.

Destructive migrations require an explicit data preservation and rollback strategy.

## 11.6 Data Isolation

Each module must access its own tables through defined repositories or application services.

Cross-module direct writes must be prohibited.

Read-only reporting queries may cross module boundaries through approved reporting interfaces.

---

# 12. FINANCIAL LEDGER ARCHITECTURE

## 12.1 Ledger Authority

The DEYPAY ledger is the authoritative internal accounting record.

External provider balances are independent evidence used for reconciliation.

## 12.2 Ledger Structure

Recommended entities:

- LedgerAccount.
- LedgerJournal.
- LedgerEntry.
- BalanceProjection.
- Reservation.
- AccountingEvent.
- ReconciliationReference.

## 12.3 Double-Entry Requirement

Every posted journal must balance within each asset.

A journal containing multiple assets must not offset a USDT debit against an NGN credit as if they were the same unit.

Cross-asset conversions require explicit clearing, settlement and accounting treatment.

## 12.4 Ledger Account Categories

The ledger should support:

- Customer liability accounts.
- Custody asset accounts.
- Provider settlement asset accounts.
- Reserved funds.
- Conversion clearing accounts.
- Fee revenue.
- Suspense accounts.
- Operational adjustment accounts.

## 12.5 Balance Reservations

Financial operations requiring external execution must reserve customer funds before submitting the external request.

Reservations must prevent concurrent spending of the same available funds.

Move the principal and applicable customer fees from available to reserved liability accounts atomically. Settlement debits the reserved account. Confirmed conversion replaces the consumed stablecoin liability with the applicable settlement obligation.

A reservation must have:

- Reservation ID.
- Customer ID.
- Asset.
- Amount.
- Related transaction.
- Status.
- Creation time.
- Expiry policy.

Reservation expiry must not automatically release funds when an external operation may still execute.

## 12.6 Posting Rules

The ledger must:

1. Validate the accounting event.
2. Validate all account and asset relationships.
3. Check required available balances.
4. Lock or otherwise serialize affected balances.
5. Create balanced entries.
6. Update balance projections.
7. Record the journal.
8. Commit atomically.

## 12.7 Immutable Records

Posted journals must never be edited or deleted.

Corrections require compensating journals.

## 12.8 Reconciliation

The ledger must support reconciliation against:

- Custody assets.
- Provider balances.
- Blockchain deposits.
- Blockchain withdrawals.
- Bank settlement records.
- Conversion records.
- Fee records.

## 12.9 Accounting Design Dependency

The precise chart of accounts and journal mapping must be specified in the separate Ledger and Accounting Design document.

The ledger implementation must not begin with an oversimplified single-balance table.

---

# 13. WALLET ARCHITECTURE

## 13.1 Wallet Ownership

DEYPAY owns wallet profiles and customer wallet relationships.

The selected custody provider controls or safeguards the underlying blockchain infrastructure according to the approved legal and technical arrangement.

## 13.2 Wallet Provisioning

Wallet provisioning must be asynchronous.

Suggested flow:

```text
Customer KYC Approved
          |
          v
Wallet Provisioning Requested
          |
          v
Create Internal Wallet Profiles
          |
          v
Submit Provider Provisioning
          |
          v
Receive Provider References
          |
          v
Verify Provisioning Result
          |
          v
Mark Wallets Ready
```

## 13.3 Provisioning Failure

If provisioning fails:

- The customer profile remains valid.
- The wallet profile remains in a failed or pending state.
- The system records the provider error.
- Safe retries may be scheduled.
- The customer must not see an unverified deposit address.

## 13.4 Address Security

Deposit addresses must be linked to:

- Customer.
- Asset.
- Network.
- Provider.
- Address status.

An address must not be reassigned to a different customer without an explicitly approved migration procedure.

---

# 14. NUVION INTEGRATION ARCHITECTURE

## 14.1 Integration Boundary

Nuvion must be accessed exclusively through a dedicated infrastructure adapter.

The application must not scatter Nuvion-specific HTTP requests across business modules.

## 14.2 Integration Components

Proposed components:

- NuvionClient.
- NuvionEntityAdapter.
- NuvionWalletAdapter.
- NuvionDepositAdapter.
- NuvionTransferAdapter.
- NuvionFXAdapter.
- NuvionPayoutAdapter.
- NuvionWebhookAdapter.
- NuvionReconciliationAdapter.

Validate the first complete internal transfer with simulated balances before expanding this adapter set. Implement each external capability as its workflow and provider evidence become available.

## 14.3 Internal Interfaces

Example conceptual interfaces:

```rust
trait WalletProvider {
    async fn provision_wallet(
        &self,
        request: WalletProvisionRequest,
    ) -> Result<WalletProvisionResult, ProviderError>;
}

trait FxProvider {
    async fn get_quote(
        &self,
        request: FxQuoteRequest,
    ) -> Result<FxQuote, ProviderError>;
}

trait PayoutProvider {
    async fn initiate_payout(
        &self,
        request: PayoutRequest,
    ) -> Result<PayoutSubmission, ProviderError>;
}
```

These examples illustrate architectural boundaries, not finalized production interfaces.

The final Rust interfaces must specify:

- Ownership.
- Async trait implementation strategy.
- Error types.
- Timeouts.
- Idempotency.
- Request context.
- Provider capabilities.

## 14.4 Provider References

All Nuvion identifiers must be mapped to DEYPAY identifiers.

The application must never expose Nuvion's internal identifiers as the primary customer-facing transaction identity.

## 14.5 Capability Validation

Each Nuvion capability must be categorized as:

- Documented.
- Sandbox tested.
- Production enabled.
- Contractually approved.
- Regulatory approved.
- Unavailable.

A documented API endpoint does not automatically imply production authorization for DEYPAY.

## 14.6 Provider Replacement

The architecture must allow an alternative provider to be integrated through the same interface.

Provider migration must consider:

- Existing customer addresses.
- Custody arrangements.
- Open transactions.
- Historical provider references.
- Customer asset migration.
- Reconciliation.
- Legal responsibilities.

Provider abstraction simplifies replacement but does not eliminate operational migration requirements.

---

# 15. PROVIDER ABSTRACTION LAYER

## 15.1 Purpose

The provider abstraction layer separates DEYPAY's business operations from external infrastructure implementations.

## 15.2 Provider Interfaces

Recommended interfaces:

- IdentityVerificationProvider.
- WalletProvider.
- DepositMonitoringProvider.
- StablecoinTransferProvider.
- FxProvider.
- BankAccountVerificationProvider.
- BankPayoutProvider.
- RiskScreeningProvider.
- NotificationProvider.

## 15.3 Capability Registry

The system should maintain a capability registry containing:

- Provider name.
- Supported service.
- Supported asset.
- Supported network.
- Supported currency corridor.
- Supported country.
- Environment.
- Operational status.
- Activation status.

## 15.4 Routing

Provider selection must be explicit and controlled.

The system must not automatically switch a financial operation to another provider after an uncertain execution outcome.

Provider failover requires reconciliation and transaction-specific safeguards.

---

# 16. DEPOSIT ARCHITECTURE

## 16.1 Deposit Lifecycle

```text
Blockchain Deposit
        |
        v
Custody / Monitoring Provider
        |
        v
Verified Provider Event
        |
        v
Deposit Detection
        |
        v
Transaction Verification
        |
        v
Confirmation Tracking
        |
        v
Risk Screening
        |
        v
Ledger Credit
        |
        v
Customer Notification
```

## 16.2 Deposit Uniqueness

The system must prevent duplicate credits.

Uniqueness should be based on the appropriate chain-specific deposit identity.

Depending on the blockchain, this may include:

- Network.
- Transaction hash.
- Log index or instruction index.
- Token contract.
- Recipient address.
- Provider event reference.

Transaction hash alone may not uniquely identify every token transfer within a blockchain transaction.

## 16.3 Confirmation Policy

Each network must have a configured confirmation or finality policy.

The system must not assume all networks have identical settlement behavior.

## 16.4 Risk Holds

Deposits may be placed under review before becoming available.

The ledger must distinguish detected funds from spendable customer balances.

---

# 17. WITHDRAWAL ARCHITECTURE

## 17.1 Withdrawal Lifecycle

```text
Withdrawal Request
        |
        v
Validate Customer
        |
        v
Validate Address / Network
        |
        v
Compliance Checks
        |
        v
Authorize Transaction
        |
        v
Reserve Balance
        |
        v
Submit to Provider
        |
        v
Track Blockchain Status
        |
        v
Finalize Accounting
```

## 17.2 Execution Safety

A withdrawal request must have one internal operation identifier.

Provider submissions must use idempotency mechanisms where supported.

Before dispatch, commit the operation, its stable submission reference and its outbox job. A restarted worker must first resolve a possibly accepted submission by that reference or external evidence. Without reliable lookup or provider idempotency, suspend dispatch for reconciliation. After an uncertain dispatch, resume submission only when lookup or reconciliation establishes non-execution. Reuse the persisted reference where the provider supports idempotency.

## 17.3 Unknown Outcomes

If the provider times out:

- Preserve the transaction reference.
- Maintain appropriate reservations.
- Query provider status.
- Reconcile the operation.
- Do not blindly resubmit.

## 17.4 Withdrawal Completion

A withdrawal must be considered completed only after the configured execution and settlement criteria have been satisfied.

---

# 18. INTERNAL TRANSFER ARCHITECTURE

## 18.1 Design

Internal transfers are ledger operations.

They should not require blockchain transactions when both users hold claims within the same approved custodial arrangement.

## 18.2 Processing

```text
Sender
   |
   v
Recipient Resolution
   |
   v
Eligibility Check
   |
   v
Balance Validation
   |
   v
Transaction Authorization
   |
   v
Atomic Ledger Posting
   |
   +---- Debit Sender
   |
   +---- Credit Recipient
   |
   v
Transaction Completed
```

## 18.3 Concurrency

Concurrent transfer requests must not overspend a customer's balance.

The ledger must use appropriate database locking or serializable transaction strategies.

## 18.4 Recipient Resolution

Username and phone-number lookup must resolve to internal customer identifiers.

Phone-number lookup must respect privacy and discoverability settings.

## 18.5 Regulatory Gate

The technical implementation may be tested using simulated balances.

Production activation requires confirmation of the legal and regulatory model for platform-facilitated internal transfers.

---

# 19. PAY ANYONE ARCHITECTURE

## 19.1 Objective

Pay Anyone enables a customer to use stablecoin balances to pay a third-party Nigerian bank account.

The recipient receives NGN.

## 19.2 Logical Components

- Bank Directory Service.
- Account Resolution Service.
- FX Quote Service.
- Payment Authorization Service.
- Ledger Reservation Service.
- Conversion Orchestrator.
- Bank Payout Adapter.
- Payment Status Tracker.
- Reconciliation Service.

## 19.3 Processing Flow

```text
Customer Enters Bank Details
             |
             v
      Resolve Account Name
             |
             v
      Customer Confirms
             |
             v
       Request FX Quote
             |
             v
       Customer Authorizes
             |
             v
      Validate and Reserve
             |
             v
       Execute Conversion
             |
             v
       Initiate NGN Payout
             |
             v
       Verify Payout Status
             |
             v
       Finalize Accounting
             |
             v
       Generate Receipt
```

## 19.4 Transaction Consistency

Pay Anyone spans multiple external systems.

It cannot be implemented as one ordinary database transaction covering the conversion provider and Nigerian banking network.

The application must use a durable orchestration workflow.

## 19.5 Compensation

When a step fails, the system must determine whether compensation is possible.

For example:

- A failed pre-execution validation may release a reservation.
- A confirmed rejected payout may permit a refund or reversal workflow.
- A completed stablecoin conversion followed by a failed bank payout may require NGN settlement recovery or a separate conversion.
- An uncertain payout must not automatically trigger a second payout.

Compensation must reflect the actual external financial state.

## 19.6 Liquidity

The architecture must support monitoring of:

- Available NGN settlement liquidity.
- Conversion provider balances.
- Pending payouts.
- Unsettled obligations.
- Funding requirements.

The exact liquidity model depends on the approved providers.

---

# 20. CONVERSION AND LIQUIDITY ARCHITECTURE

## 20.1 Conversion Engine

The FX module must separate:

- Quote retrieval.
- Quote validation.
- Customer acceptance.
- Execution.
- Settlement.
- Reconciliation.

## 20.2 Quote Lifecycle

Quotes must include:

- Internal quote ID.
- Provider quote ID.
- Source asset.
- Destination asset.
- Source amount.
- Destination amount.
- Rate.
- Fees.
- Expiry.
- Status.

## 20.3 Quote Expiry

An expired quote must not be executed.

The customer must receive a new quote.

## 20.4 Liquidity Controls

The system should support:

- Minimum settlement balance alerts.
- Conversion limits.
- Provider liquidity status.
- Payout exposure limits.
- Reconciliation of liquidity movements.

## 20.5 Pricing

Customer pricing must be managed by DEYPAY.

Provider fees and exchange rates may be inputs, but DEYPAY must calculate and disclose the final customer quote according to approved pricing rules.

---

# 21. TRANSACTION ORCHESTRATION

## 21.1 Purpose

Transaction orchestration coordinates multi-step financial operations.

## 21.2 Workflow Persistence

Every long-running financial workflow must persist:

- Operation ID.
- Operation type.
- Current state.
- Provider references.
- Retry count.
- Last error.
- Next action.
- Timestamps.
- Reconciliation status.

## 21.3 State Transitions

Persist operation-specific states and their evidence as defined in 05-LEDGER-ACCOUNTING.md §25. Record verified transitions and their postings atomically. Reject invalid or stale transitions and map customer-visible status through 04-API-SPECIFICATION.md §38.

## 21.4 Recovery

After a process crash or deployment restart, the worker must resume eligible incomplete operations.

It must not lose the financial operation state.

---

# 22. EVENT-DRIVEN PROCESSING

## 22.1 Event Model

The system should publish internal domain events.

Examples:

- CustomerRegistered.
- KycApproved.
- WalletProvisioningRequested.
- DepositDetected.
- DepositCredited.
- WithdrawalRequested.
- WithdrawalCompleted.
- InternalTransferCompleted.
- BankPaymentSubmitted.
- BankPaymentCompleted.
- ReconciliationMismatchDetected.

## 22.2 Transactional Outbox

Events associated with committed database changes must be recorded in the same database transaction.

A background worker then processes or publishes them.

This prevents committed financial operations from losing required follow-up work.

## 22.3 Delivery Semantics

The system should assume at-least-once event delivery.

Consumers must be idempotent.

Webhook, polling and reconciliation consumers resolve the same external transfer or settlement leg to one economic event before invoking the posting engine. A delivery or request identifier alone cannot identify that event.

Exactly-once delivery across independent external systems must not be assumed.

## 22.4 Dead-Letter Handling

Repeatedly failing jobs must be moved to an investigation state or dead-letter mechanism.

Financial jobs must never be silently discarded.

---

# 23. WEBHOOK ARCHITECTURE

## 23.1 Inbound Webhooks

Providers may send events for:

- Deposits.
- Withdrawals.
- Conversions.
- Payouts.
- Account provisioning.
- Verification results.

## 23.2 Webhook Processing

The webhook endpoint must:

1. Receive the raw request.
2. Validate authentication or signature.
3. Validate payload structure.
4. Identify the provider.
5. Store the event.
6. Deduplicate the event.
7. Acknowledge receipt according to the provider's protocol.
8. Process the event asynchronously.
9. Update the relevant workflow after verification.

## 23.3 Security

Webhook secrets must be protected.

Where supported, signatures must be verified against the raw request body using the provider's documented algorithm.

Replay protection must be implemented according to provider capabilities.

## 23.4 Reconciliation

Webhooks must not be the only source of financial transaction status.

The system must be able to query providers or retrieve settlement records to recover missed events.

---

# 24. AUTHENTICATION AND AUTHORIZATION

## 24.1 Authentication

The backend must support:

- Email/password authentication.
- Verified contact methods.
- Session management.
- Password recovery.
- MFA.
- Transaction step-up authentication.

## 24.2 Session Security

Recommended controls:

- Opaque, server-side PWA sessions with Secure, HttpOnly and SameSite=Lax cookies (04-API-SPECIFICATION.md §6).
- CSRF protection where required.
- Session expiry.
- Session revocation.
- Device management.
- Login rate limiting.

## 24.3 Authorization

Every protected API endpoint must verify:

- Authentication.
- Resource ownership.
- Required permission.
- Account status.
- Financial eligibility where applicable.

## 24.4 Financial Authorization

A valid login session alone must not automatically authorize every financial operation.

High-risk operations should require transaction-specific confirmation or step-up authentication.

---

# 25. KYC AND COMPLIANCE ARCHITECTURE

## 25.1 Eligibility Service

The backend must expose a central eligibility check.

Financial modules must use this service before executing restricted operations.

## 25.2 Compliance Workflow

The system must support:

- KYC submission.
- Identity verification.
- Sanctions screening.
- Risk classification.
- Manual review.
- Account restriction.
- Ongoing monitoring.

## 25.3 Data Handling

Identity documents must be stored in protected storage or retained by an approved verification provider under the applicable legal arrangements.

Sensitive information must not be unnecessarily duplicated.

## 25.4 Compliance Decisions

Compliance decisions must be auditable.

The system must record:

- Decision.
- Reason.
- Actor.
- Timestamp.
- Supporting references.

---

# 26. SECURITY ARCHITECTURE

## 26.1 Threat Model

The system must account for:

- Account takeover.
- Credential stuffing.
- Unauthorized withdrawals.
- API abuse.
- Provider credential theft.
- Webhook forgery.
- Duplicate financial execution.
- Insider misuse.
- Database compromise.
- Sensitive information exposure.
- Supply-chain vulnerabilities.

## 26.2 Application Security

Controls must include:

- Input validation.
- Output encoding.
- Authentication.
- Authorization.
- Rate limiting.
- Secure error handling.
- Dependency scanning.
- Secret scanning.
- Secure build processes.

## 26.3 Database Security

Controls must include:

- Restricted database users.
- Encrypted connections.
- Managed backups.
- Network restrictions.
- Audit logging.
- Migration controls.

## 26.4 Financial Security

Controls must include:

- Idempotency.
- Reservations.
- Atomic ledger posting.
- Transaction authorization.
- Provider status verification.
- Reconciliation.
- Maker-checker for sensitive operations.

## 26.5 Key Management

DEYPAY must not casually store blockchain private keys in its ordinary application database.

The custody and signing arrangement must be contractually and technically approved.

## 26.6 Incident Response

The platform must have procedures for:

- Credential compromise.
- Provider outage.
- Suspicious withdrawals.
- Reconciliation discrepancies.
- Unauthorized administrative access.
- Customer asset incidents.

---

# 27. API ARCHITECTURE

## 27.1 API Style

The initial backend will expose versioned REST APIs.

Base path:

/api/v1

## 27.2 API Responsibilities

HTTP handlers should:

- Authenticate requests.
- Validate input.
- Invoke application services.
- Translate results.
- Return standardized responses.

Handlers must not contain complex financial business logic.

## 27.3 Response Structure

Responses should use consistent schemas.

Errors should include:

- Stable error code.
- Human-readable message.
- Request correlation ID.
- Optional field-level validation details.

Sensitive internal errors must not be exposed.

## 27.4 Idempotency

Financial write endpoints must support idempotency keys.

A repeated request with the same key and identical payload must not execute the financial operation twice.

A repeated key with a conflicting payload must be rejected.

Idempotency records must persist for an appropriate duration.

## 27.5 API Documentation

OpenAPI documentation must be maintained alongside the implementation.

Generated API clients may be used by the frontend.

---

# 28. FRONTEND AND PWA ARCHITECTURE

## 28.1 Frontend Independence

The frontend must communicate with DEYPAY's own API.

It must not depend directly on Nuvion's privileged API.

## 28.2 Suggested Application Sections

- Authentication.
- Onboarding.
- Dashboard.
- Wallets.
- Deposits.
- Withdrawals.
- Transfers.
- Payments.
- Conversions.
- Transaction history.
- Profile.
- Support.

## 28.3 State Management

The frontend must distinguish:

- Server state.
- Form state.
- Authentication state.
- UI state.

Financial balances must always originate from backend-controlled data.

## 28.4 PWA Security

Sensitive financial information must not be indiscriminately cached.

Offline mode must not permit execution of financial operations without backend confirmation.

## 28.5 Transaction UX

Financial screens must clearly display:

- Recipient.
- Asset.
- Amount.
- Fees.
- Exchange rate.
- Total debit.
- Transaction status.

The interface must never display a payment as completed solely because the request was submitted.

---

# 29. ADMINISTRATION ARCHITECTURE

## 29.1 Admin Interface

The administration interface should be logically separated from the customer-facing application.

## 29.2 Administrative Roles

The backend must enforce role-based permissions.

## 29.3 Audit Logging

Every sensitive administrative action must record:

- Actor.
- Action.
- Target.
- Reason.
- Timestamp.
- Outcome.
- Relevant correlation ID.

## 29.4 Financial Adjustments

Administrators must not directly edit customer balances.

Financial adjustments must use approved ledger operations.

Sensitive adjustments must require maker-checker approval.

---

# 30. NOTIFICATIONS ARCHITECTURE

## 30.1 Event-Driven Notifications

Notifications should be generated from committed business events.

## 30.2 Channels

Supported channels may include:

- In-app.
- Email.
- Push.
- SMS.

## 30.3 Delivery

Notification delivery failures must not roll back completed financial transactions.

Failed notifications should be retried independently.

## 30.4 Sensitive Data

Notifications must not expose private keys, full identity documents or unnecessary financial details.

---

# 31. RECONCILIATION ARCHITECTURE

## 31.1 Purpose

Reconciliation ensures that DEYPAY's internal records match actual external financial activity.

## 31.2 Reconciliation Sources

- Internal ledger.
- Provider transactions.
- Custody balances.
- Blockchain transaction records.
- Nigerian payout records.
- Conversion records.
- Settlement statements.

## 31.3 Matching

Matching should use:

- Internal transaction ID.
- Provider transaction ID.
- Blockchain reference.
- Asset.
- Amount.
- Network.
- Timestamp.
- Settlement status.

## 31.4 Discrepancies

The system must detect:

- Missing transactions.
- Duplicate transactions.
- Amount mismatches.
- Fee mismatches.
- Unexpected pending states.
- Balance mismatches.
- Missing settlement confirmations.

## 31.5 Reconciliation Cases

Every discrepancy must have:

- Case ID.
- Category.
- Severity.
- Status.
- Assigned owner.
- Evidence.
- Resolution notes.

## 31.6 Financial Safeguard

Critical reconciliation discrepancies must trigger operational alerts.

The system must support temporarily disabling affected transaction capabilities when necessary to protect customer assets.

---

# 32. OBSERVABILITY ARCHITECTURE

## 32.1 Logging

Use structured logs.

Every request and financial workflow should carry a correlation ID.

## 32.2 Metrics

Monitor:

- API latency.
- Error rates.
- Database performance.
- Queue backlog.
- Provider response times.
- Transaction completion rates.
- Failed transactions.
- Reconciliation mismatches.

## 32.3 Tracing

Distributed tracing should connect:

- Incoming API request.
- Application service.
- Database operation.
- Background job.
- Provider request.

## 32.4 Financial Monitoring

Financial alerts must be separate from ordinary technical alerts.

Examples:

- Ledger imbalance.
- Duplicate execution attempt.
- Unreconciled payout.
- Unexpected custody balance.
- Liquidity shortage.

---

# 33. INFRASTRUCTURE AND DEPLOYMENT

## 33.1 Initial Deployment

Recommended components:

- API application.
- Background worker.
- Managed PostgreSQL.
- Redis where required.
- Object storage.
- Secrets management.
- Monitoring.
- Backup service.

## 33.2 Network Security

Database and internal services must not be publicly accessible without a justified need.

## 33.3 TLS

All production external communication must use encrypted transport.

## 33.4 CI/CD

The deployment pipeline must:

1. Build the application.
2. Run formatting checks.
3. Run linting.
4. Run unit tests.
5. Run integration tests.
6. Scan dependencies.
7. Build deployment artifacts.
8. Deploy to staging.
9. Run smoke tests.
10. Require approval for production deployment.

## 33.5 Database Migrations

Migrations must be controlled and versioned.

Production deployment must not depend on unreviewed destructive migrations.

---

# 34. CONFIGURATION AND SECRETS

## 34.1 Configuration

Configuration must be environment-specific.

Examples:

- Database connection.
- Redis connection.
- API port.
- Provider endpoints.
- Enabled assets.
- Enabled networks.
- Transaction limits.
- Feature flags.

## 34.2 Secrets

Secrets must include:

- Provider API credentials.
- Webhook signing secrets.
- Database credentials.
- Encryption keys.
- Messaging credentials.

Secrets must not be stored in source control.

## 34.3 Feature Flags

The system should support controlled activation of:

- Stablecoin deposits.
- Stablecoin withdrawals.
- Internal transfers.
- NGN funding.
- Pay Anyone.
- Specific blockchain networks.
- Specific providers.

Feature activation must respect compliance approval and operational readiness.

---

# 35. RELIABILITY AND FAILURE RECOVERY

## 35.1 Provider Outages

Provider outages must not corrupt internal financial records.

## 35.2 Database Failures

Incomplete database transactions must roll back safely.

## 35.3 Worker Crashes

Durable jobs must remain recoverable.

## 35.4 Duplicate Events

Repeated events must not cause duplicate ledger postings.

## 35.5 Unknown External Outcomes

The system must preserve the operation state and reconcile before retrying.

## 35.6 Reversals

Reversals must use explicit accounting events.

Historical financial entries must not be deleted.

---

# 36. PERFORMANCE AND SCALABILITY

## 36.1 Initial Targets

Suggested initial targets:

- 95th percentile internal API latency below 500 ms for non-provider-dependent operations.
- 99.9% monthly core backend availability target.
- No unexplained ledger imbalances.
- No duplicate financial execution under tested retry scenarios.

These are design targets requiring load and recovery validation.

## 36.2 Horizontal Scaling

API application instances should be stateless where practical.

Persistent financial state must remain in the database.

## 36.3 Database Scaling

Scaling options may include:

- Query optimization.
- Indexing.
- Connection pooling.
- Read replicas for suitable workloads.
- Partitioning when justified.

Financial correctness must not be sacrificed for performance.

## 36.4 Background Workers

Worker concurrency must be configurable.

Financial jobs must respect ordering and concurrency requirements.

---

# 37. TESTING ARCHITECTURE

## 37.1 Unit Tests

Test domain rules independently.

## 37.2 Integration Tests

Test application services with PostgreSQL and external provider mocks.

## 37.3 Contract Tests

Test provider adapters against documented API contracts.

## 37.4 Ledger Tests

Test:

- Balanced journals.
- Atomic postings.
- Concurrent transfers.
- Reservation handling.
- Duplicate requests.
- Reversals.
- Precision.

## 37.5 Failure Tests

Simulate:

- Provider timeout.
- Duplicate webhook.
- Worker crash.
- Database rollback.
- Partial external execution.
- Delayed settlement.
- Missing webhook.

## 37.6 Security Tests

Test authentication, authorization, session handling, webhook verification and administrative controls.

## 37.7 End-to-End Tests

Test complete customer financial journeys through the backend.

## 37.8 Production Testing

Controlled live testing must only occur after the relevant provider and legal approvals.

---

# 38. DEVELOPMENT ENVIRONMENTS

## 38.1 Local Development

Local development should use:

- Rust toolchain.
- PostgreSQL.
- Redis where required.
- Mock provider adapters.
- Docker Compose.

## 38.2 Staging

Staging should resemble production where practical.

It must use test credentials and non-production customer data.

## 38.3 Production

Production must use:

- Approved provider credentials.
- Secure secrets.
- Monitoring.
- Backups.
- Restricted access.
- Audited deployment procedures.

---

# 39. REPOSITORY STRUCTURE

Recommended repository structure:

```text
deypay/
|
+-- docs/
|   +-- 01-FRS.md
|   +-- 02-SYSTEM-ARCHITECTURE.md
|   +-- 03-DATABASE-DESIGN.md
|   +-- 04-API-SPECIFICATION.md
|   +-- 05-LEDGER-ACCOUNTING.md
|   +-- 06-SECURITY-COMPLIANCE.md
|   +-- 07-TEST-STRATEGY.md
|   +-- 08-IMPLEMENTATION-PLAN.md
|   +-- adr/
|
+-- backend/
|   +-- Cargo.toml
|   +-- Cargo.lock
|   +-- crates/
|   |   +-- deypay-domain/
|   |   +-- deypay-application/
|   |   +-- deypay-infrastructure/
|   |   +-- deypay-api/
|   |   +-- deypay-worker/
|   |   +-- deypay-common/
|   |
|   +-- migrations/
|   +-- tests/
|   +-- Dockerfile
|
+-- frontend/
|   +-- src/
|   +-- public/
|   +-- package.json
|
+-- infrastructure/
|   +-- docker/
|   +-- monitoring/
|   +-- deployment/
|
+-- scripts/
|
+-- .github/
|   +-- workflows/
|
+-- .env.example
+-- .gitignore
+-- README.md
```

## 39.1 Domain Crate

Contains domain entities, value objects and business invariants.

## 39.2 Application Crate

Contains use cases and application orchestration.

## 39.3 Infrastructure Crate

Contains database repositories, provider adapters and external service implementations.

## 39.4 API Crate

Contains Axum routes, middleware, request validation and response serialization.

## 39.5 Worker Crate

Contains asynchronous job processing, reconciliation and scheduled operations.

## 39.6 Common Crate

Contains carefully selected shared primitives.

The common crate must not become a dumping ground for unrelated business logic.

---

# 40. CODING AND ENGINEERING STANDARDS

## 40.1 Rust Standards

Use:

- rustfmt.
- Clippy.
- Explicit error types.
- Strongly typed identifiers.
- Clear module boundaries.
- Safe asynchronous programming.
- Structured tracing.

## 40.2 Error Handling

Avoid uncontrolled panics in request and worker processing.

Errors must be classified and propagated appropriately.

## 40.3 Financial Types

Use precise money representations.

Never use f32 or f64 for financial accounting.

## 40.4 Database Access

Use parameterized queries.

Use transactions for financial operations.

Avoid unbounded database queries.

## 40.5 Testing

Financial business logic must have automated tests.

Every bug affecting financial integrity must receive a regression test.

## 40.6 Documentation

Public APIs, domain invariants and financial workflows must be documented.

## 40.7 Code Review

Changes to ledger, custody, payout, security and authorization logic require elevated review.

---

# 41. ARCHITECTURE DECISION RECORDS

Significant decisions must be recorded in the docs/adr directory.

Recommended initial ADRs:

ADR-001: Rust as Backend Language.

ADR-002: Modular Monolith Architecture.

ADR-003: PostgreSQL as Primary Database.

ADR-004: Double-Entry Financial Ledger.

ADR-005: Provider Abstraction Architecture.

ADR-006: Nuvion Integration Strategy.

ADR-007: Durable Background Job Processing.

ADR-008: Stablecoin-First Wallet Model.

ADR-009: No General-Purpose NGN Wallet in MVP.

ADR-010: Internal Ledger-Based Transfers.

ADR-011: Pay Anyone Payment Orchestration.

ADR-012: Backend-First Development Strategy.

Each ADR should contain:

- Context.
- Decision.
- Alternatives.
- Rationale.
- Consequences.
- Status.
- Date.

---

# 42. TECHNICAL RISKS

## 42.1 Provider Lock-In

Mitigation: Provider-neutral interfaces and internal financial records.

## 42.2 Financial Race Conditions

Mitigation: Database transactions, locking and ledger invariants.

## 42.3 Duplicate External Execution

Mitigation: Idempotency and reconciliation.

## 42.4 Webhook Reliability

Mitigation: Persistent event storage and provider polling.

## 42.5 Liquidity Failure

Mitigation: Settlement monitoring and transaction limits.

## 42.6 Complex Failure Recovery

Mitigation: Durable orchestration and explicit state machines.

## 42.7 Custody Provider Failure

Mitigation: Approved custody arrangements and recovery planning.

## 42.8 Premature Scaling Complexity

Mitigation: Modular monolith and measured infrastructure growth.

---

# 43. REGULATORY DEPENDENCIES

The architecture must support the operating model approved for Nigeria.

Before production, the business must confirm:

- Applicable licences and approvals.
- Permitted custody structure.
- Stablecoin activity permissions.
- Customer asset safeguarding.
- Internal transfer permissions.
- Fiat conversion arrangements.
- NGN bank payout permissions.
- AML and sanctions responsibilities.
- Data protection obligations.
- Regulatory reporting obligations.

The architecture must support the legally responsible entity's compliance duties.

A technical integration with Nuvion does not independently establish legal permission to conduct regulated financial activities.

---

# 44. IMPLEMENTATION SEQUENCE

The recommended implementation order is:

## Stage 1: Repository and Tooling

- Initialize Rust workspace.
- Configure formatting.
- Configure linting.
- Configure CI.
- Configure PostgreSQL.
- Configure environment management.
- Configure structured logging.

## Stage 2: Domain Foundation

- Define money types.
- Define identifiers.
- Define domain errors.
- Define transaction states.
- Define asset and network models.

## Stage 3: Identity and Customer

- Registration.
- Authentication.
- Profiles.
- Username management.
- Sessions.
- Authorization.

## Stage 4: Financial Ledger

- Ledger accounts.
- Journals.
- Entries.
- Reservations.
- Balance projections.
- Concurrency controls.
- Financial tests.

## Stage 5: Wallets and KYC

- Wallet profiles.
- KYC workflows.
- Eligibility.
- Wallet provisioning.
- Provider references.

## Stage 6: Provider Integration

- Provider interfaces.
- Nuvion adapter.
- Mock adapters.
- Webhook infrastructure.
- Provider event persistence.

## Stage 7: Deposits and Withdrawals

- Deposit processing.
- Confirmation tracking.
- Ledger crediting.
- Withdrawal authorization.
- Reservation.
- Provider execution.

## Stage 8: Internal Transfers

- Username resolution.
- Recipient verification.
- Atomic ledger transfers.
- Notifications.

## Stage 9: FX and Pay Anyone

- Quote engine.
- Bank account verification.
- Payment orchestration.
- NGN payout integration.
- Liquidity monitoring.
- Reconciliation.

## Stage 10: Administration and Compliance

- Admin authentication.
- KYC review.
- Transaction investigation.
- Risk controls.
- Reconciliation dashboard.
- Audit reporting.

## Stage 11: Complete Backend Testing

- Unit testing.
- Integration testing.
- Financial concurrency testing.
- Provider failure testing.
- Security testing.
- End-to-end testing.

## Stage 12: Frontend Development

- PWA.
- Authentication.
- Dashboard.
- Wallets.
- Payments.
- Transactions.
- Profile.
- Support.

## Stage 13: Controlled Production Launch

- Compliance approval.
- Provider production approval.
- Security assessment.
- Operational readiness.
- Pilot testing.
- Gradual rollout.

---

# 45. ARCHITECTURE ACCEPTANCE CRITERIA

The architecture will be accepted when:

1. Core modules have clear ownership boundaries.
2. Business logic is independent of concrete providers.
3. The ledger enforces double-entry accounting.
4. Financial operations are atomic where required.
5. External financial workflows use durable state machines.
6. Financial write operations are idempotent.
7. Provider webhooks are verified and deduplicated.
8. Missing events can be recovered.
9. Provider timeouts do not cause duplicate payouts.
10. Internal transfers prevent concurrent overspending.
11. External balances can be reconciled.
12. Financial adjustments are auditable.
13. API authorization is enforced.
14. Sensitive credentials are protected.
15. The backend can be tested without production provider access.
16. Nuvion can be replaced or supplemented through provider interfaces.
17. Production financial features are controlled by appropriate approval gates.
18. The backend supports the complete approved MVP requirements.

---

# 46. OPEN TECHNICAL DECISIONS

The following require resolution during detailed design.

## 46.1 Custody Model

- Who legally holds customer stablecoins?
- Who controls signing keys?
- Are customer addresses segregated?
- Are funds pooled or individually controlled?
- What recovery arrangements exist?

## 46.2 Supported Networks

- Which USDC networks launch first?
- Which USDT networks launch first?
- What confirmation policies apply?
- What are the provider's network fees?

## 46.3 Nigerian Payments

- Which provider resolves Nigerian bank accounts?
- Which provider executes NGN payouts?
- Is Nuvion the primary conversion provider?
- Is separate NGN liquidity required?
- How are failed payouts refunded?

## 46.4 Infrastructure

- Which cloud provider will host DEYPAY?
- Which secrets manager will be used?
- Which monitoring platform will be used?
- Which object storage service will be used?

## 46.5 Financial Operations

- What is the approved chart of accounts?
- What is the settlement model?
- What are the initial transaction limits?
- What are the reconciliation tolerances?
- Which financial actions require maker-checker approval?

These decisions must be documented before implementing the affected production capabilities.

---

# 47. FINAL ARCHITECTURE DEFINITION

DEYPAY will be built as an independent financial technology platform using Rust and a modular monolith architecture.

The backend will own the complete application business logic, customer identity, wallet profiles, financial ledger, transaction orchestration, payment workflows, compliance integration, administration and reconciliation.

PostgreSQL will provide the authoritative transactional data store.

A double-entry ledger will provide the foundation for all customer financial balances and movements.

Financial operations involving external systems will use durable state machines, balance reservations, idempotency, provider status verification and reconciliation.

Nuvion will be integrated as a preferred external infrastructure provider through replaceable adapters.

Nigerian bank payment providers, custody infrastructure, identity verification providers and other external services will be integrated through similarly isolated interfaces.

The frontend will be a responsive Progressive Web Application communicating exclusively with DEYPAY-controlled APIs.

The backend will be implemented and tested before primary frontend development.

The architecture will prioritize financial correctness, security, provider independence, operational transparency and controlled scalability.

Future products—including gift cards, prediction markets, stocks, investment services and additional payment capabilities—will be introduced as separate business modules using the established customer, wallet, ledger and payment foundations where legally and technically appropriate.

**FINAL ARCHITECTURAL PRINCIPLE**

DEYPAY controls its product.

DEYPAY controls its business logic.

DEYPAY maintains its own financial accounting.

External providers supply specialized infrastructure.

No external provider should become an unavoidable dependency of the entire application.

Every financial operation must be secure, traceable, recoverable, reconciled and auditable.

---

**END OF DOCUMENT**

**DEYPAY SYSTEM ARCHITECTURE DOCUMENT**

**Document ID:** DP-ARCH-002  
**Version:** 1.0  
**Status:** Proposed Architecture Baseline