# DEYPAY (DP)
# DATABASE DESIGN SPECIFICATION (DDS)
## PostgreSQL Data Architecture, Entity Relationships, Financial Integrity and Persistence Design

**Document ID:** DP-DB-003  
**Version:** 1.0  
**Status:** Proposed Engineering Baseline – Pending Implementation Review  
**Product:** DEYPAY  
**Initial Market:** Nigeria  
**Database Engine:** PostgreSQL 16 or newer, subject to deployment compatibility  
**Backend:** Rust, Axum, SQLx  
**Architecture:** Modular Monolith  
**Date:** 17 November 2026  
**Classification:** Internal – Engineering and Financial Infrastructure

**Related Documents:**
- DP-001: Functional Requirements Specification
- DP-002: System Architecture Document
- DP-005: Financial Ledger and Accounting Design Specification (05-LEDGER-ACCOUNTING.md)

---

# DOCUMENT CONTROL

## 1. Purpose

This document defines the proposed database architecture for DEYPAY.

It establishes the data structures required to support the complete MVP, including customer identity, authentication, KYC, custodial stablecoin wallets, deposits, withdrawals, internal transfers, currency conversion, Nigerian bank payouts, provider integrations, transaction processing, compliance, administration, reporting and reconciliation.

It is intended to be used as a direct reference for database migration development, Rust domain modeling, SQLx repositories, automated integration testing and operational database management.

This specification is a logical and physical design baseline. It is not a complete executable SQL migration package. Final migration scripts must be produced, reviewed and tested against the approved accounting model and provider contracts.

## 2. Design Authority

The FRS defines required product behavior.

The SAD defines the system's architectural boundaries.

This DDS defines how data will be represented and protected.

05-LEDGER-ACCOUNTING.md defines the reservation model, illustrative journal mappings and posting rules. Finance approves the final accounts and route-specific mappings.

If the documents conflict, engineering must record the conflict and resolve it before implementing the affected functionality.

---

# TABLE OF CONTENTS

1. Executive Summary
2. Database Design Objectives
3. Core Design Principles
4. PostgreSQL Architecture
5. Database Schemas
6. Naming and Data Type Standards
7. Entity Relationship Overview
8. Identity and Authentication Tables
9. Customer and Username Tables
10. KYC and Compliance Tables
11. Asset and Blockchain Network Tables
12. Wallet and Address Tables
13. Financial Ledger Tables
14. Balance Reservation Tables
15. Transaction Tables
16. Stablecoin Deposit Tables
17. Stablecoin Withdrawal Tables
18. Internal Transfer Tables
19. FX Quote and Conversion Tables
20. Nigerian Bank and Beneficiary Tables
21. Pay Anyone Tables
22. Provider Integration Tables
23. Webhook and Event Processing Tables
24. Notifications and Payment Links
25. Administration and Audit Tables
26. Reconciliation Tables
27. Support and Dispute Tables
28. Relationship and Foreign Key Rules
29. Financial Integrity Constraints
30. Transaction Isolation and Concurrency
31. Idempotency Design
32. Indexing and Query Optimization
33. Database Security
34. Sensitive Data Protection
35. Data Retention and Privacy
36. Backup and Disaster Recovery
37. Migration Strategy
38. SQLx Integration
39. Testing and Validation
40. Operational Monitoring
41. Database Implementation Sequence
42. Design Risks and Mitigations
43. Open Design Decisions
44. Database Acceptance Criteria
45. Final Database Architecture Definition

---

# 1. EXECUTIVE SUMMARY

DEYPAY will use PostgreSQL as its primary authoritative database.

The database must support two distinct responsibilities:

**Operational persistence:** customer accounts, profiles, wallets, provider integrations, transaction workflows and application state.

**Financial persistence:** immutable accounting journals, ledger entries, reservations, balances, settlement records and reconciliation evidence.

The system must never treat a simple editable wallet balance field as the authoritative financial record.

Every financial movement must be represented by a valid accounting event and corresponding ledger entries.

PostgreSQL transactions will provide atomicity for operations that can be completed within DEYPAY's own database.

Operations involving external providers will use durable workflow records, explicit state transitions, idempotency, verified provider results and reconciliation.

The database will be designed to support multiple providers without making Nuvion's identifiers or data model the foundation of DEYPAY's internal system.

All primary business entities will use DEYPAY-generated identifiers.

---

# 2. DATABASE DESIGN OBJECTIVES

The database must:

1. Preserve financial correctness.
2. Support the approved DEYPAY MVP.
3. Prevent duplicate financial postings.
4. Support concurrent transactions safely.
5. Maintain immutable accounting history.
6. Track all external provider operations.
7. Support customer identity and verification.
8. Support USDC and USDT wallet profiles.
9. Support multiple blockchain networks.
10. Support internal DEYPAY transfers.
11. Support NGN funding and payouts.
12. Support conversion workflows.
13. Support administrative investigations.
14. Support financial reconciliation.
15. Protect sensitive customer information.
16. Support controlled schema evolution.
17. Remain compatible with Rust and SQLx.
18. Support future product expansion.

---

# 3. CORE DESIGN PRINCIPLES

## 3.1 Financial Correctness First

Financial integrity takes precedence over performance optimizations.

## 3.2 Double-Entry Accounting

Every posted financial journal must balance within its accounting asset.

## 3.3 Immutable Financial History

Posted journal entries must not be modified or deleted.

Corrections must use compensating journals.

## 3.4 Strong Internal Identifiers

DEYPAY-generated IDs must remain stable independently of external providers.

## 3.5 Explicit Asset Identity

USDC and USDT must be represented as distinct assets.

Blockchain network and token contract identities must be modeled separately.

## 3.6 Provider Independence

Provider-specific references must be stored separately from DEYPAY's internal identifiers.

## 3.7 Idempotency

Financial operations must be protected against duplicate execution.

## 3.8 Auditable Changes

Material administrative and financial state changes must be traceable.

## 3.9 Least Privilege

Database access must be restricted according to service responsibilities.

## 3.10 Privacy by Design

Sensitive personal information must be protected and retained only as required.

---

# 4. POSTGRESQL ARCHITECTURE

## 4.1 Database Engine

Recommended: PostgreSQL 16 or newer.

The final version must be supported by the selected hosting provider and SQLx toolchain.

## 4.2 Database Deployment

The initial deployment should use one managed PostgreSQL primary instance.

High availability, replication and point-in-time recovery should be enabled according to the production risk assessment.

## 4.3 Database Ownership

The DEYPAY backend owns all application database writes.

External providers must not receive direct access to DEYPAY's production database.

## 4.4 Connection Management

Rust services must use connection pooling.

SQLx connection pools must be configured with:

- Maximum connections.
- Minimum idle connections where appropriate.
- Connection acquisition timeout.
- Connection lifetime.
- Idle timeout.
- Query timeout policies.

## 4.5 Database Extensions

Potential extensions:

- pgcrypto, if required for supported database cryptographic functions.
- pg_stat_statements for query performance monitoring, where available.

Extensions must be explicitly approved and version-controlled.

---

# 5. DATABASE SCHEMAS

The recommended logical schemas are:

| Schema | Responsibility |
|---|---|
| identity | Authentication, credentials, sessions |
| customers | Profiles, usernames, preferences |
| compliance | KYC, screening, risk cases |
| wallets | Assets, networks, wallets, addresses |
| ledger | Accounts, journals, entries, reservations |
| payments | Transfers, withdrawals, deposits, bank payments |
| fx | Quotes, conversions |
| integrations | Providers, external references, webhooks |
| operations | Notifications, support, reconciliation |
| audit | Administrative and security audit events |

PostgreSQL schemas are recommended for organizational clarity.

However, database schema separation must not be confused with independently deployed microservices.

The backend remains a modular monolith.

---

# 6. NAMING AND DATA TYPE STANDARDS

## 6.1 Naming

Use:

- snake_case for tables and columns.
- Plural table names.
- Singular primary key column `id`.
- Explicit foreign key names.
- Consistent timestamps.
- Explicit status values.

Examples:

`customers.users`

`wallets.wallet_profiles`

`ledger.journals`

`payments.bank_payments`

## 6.2 Primary Keys

Use UUID identifiers for major business entities.

Recommended Rust type:

`uuid::Uuid`

## 6.3 Timestamps

Use PostgreSQL:

`TIMESTAMPTZ`

Store timestamps in UTC.

## 6.4 Financial Amounts

Use exact representations.

Recommended financial storage:

`NUMERIC(38, 0)` for integer base-unit amounts where the asset's precision and maximum supply are compatible.

Alternative exact-decimal representations may be used where justified.

Never use floating-point database types for financial balances.

## 6.5 Asset Precision

Each asset must have an explicit precision.

For example:

USDC commonly uses six decimal places on supported networks, but actual token contract metadata must be verified.

USDT precision may vary by blockchain implementation.

The database must not assume identical token precision across all networks.

## 6.6 Status Values

Use constrained text values or PostgreSQL enums.

For evolving provider and transaction statuses, constrained text values may be easier to migrate.

Status changes must be validated by application-level state machines.

## 6.7 External References

External provider references should use bounded text fields.

Provider identifiers must never be assumed to be UUIDs unless the provider guarantees that format.

---

# 7. ENTITY RELATIONSHIP OVERVIEW

The principal relationships are:

```text
CUSTOMER
   |
   +-- AUTHENTICATION CREDENTIALS
   +-- SESSIONS
   +-- USERNAME
   +-- KYC SUBMISSIONS
   +-- WALLET PROFILES
   |      |
   |      +-- WALLET ADDRESSES
   |
   +-- LEDGER ACCOUNTS
   |      |
   |      +-- LEDGER ENTRIES
   |
   +-- TRANSACTIONS
   |      |
   |      +-- DEPOSITS
   |      +-- WITHDRAWALS
   |      +-- INTERNAL TRANSFERS
   |      +-- BANK PAYMENTS
   |      +-- CONVERSIONS
   |
   +-- BENEFICIARIES
   +-- PAYMENT LINKS
   +-- NOTIFICATIONS
   +-- SUPPORT TICKETS

PROVIDER
   |
   +-- PROVIDER ACCOUNTS
   +-- PROVIDER OPERATIONS
   +-- WEBHOOK EVENTS
   +-- RECONCILIATION RECORDS
```

This is the conceptual relationship model.

Detailed foreign key relationships are defined in subsequent sections.

---

# 8. IDENTITY AND AUTHENTICATION TABLES

## 8.1 identity.users

Purpose: Store the foundational authentication identity.

| Column | Type | Description |
|---|---|---|
| id | UUID PK | Internal user identifier |
| email | CITEXT or normalized TEXT | Email |
| email_verified_at | TIMESTAMPTZ NULL | Email verification time |
| phone_e164 | TEXT NULL | Verified or pending phone |
| phone_verified_at | TIMESTAMPTZ NULL | Phone verification time |
| status | TEXT | Account status |
| created_at | TIMESTAMPTZ | Creation |
| updated_at | TIMESTAMPTZ | Last update |
| closed_at | TIMESTAMPTZ NULL | Account closure |

Constraints:

- Primary key on id.
- Unique normalized email where applicable.
- Unique verified phone according to approved policy.
- MVP registration supplies `phone_e164` and leaves `phone_verified_at` NULL.
- Controlled account status. `CONTACT_VERIFIED` requires a non-NULL `email_verified_at`. Confirming email advances accounts in either initial contact state.
- Valid timestamp relationships.

## 8.2 identity.password_credentials

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| password_hash | TEXT |
| password_updated_at | TIMESTAMPTZ |
| created_at | TIMESTAMPTZ |

Password hashes must use an approved modern algorithm such as Argon2id.

Plaintext passwords must never be stored.

## 8.3 identity.sessions

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| token_hash | TEXT |
| device_id | UUID NULL |
| ip_address | INET NULL |
| user_agent | TEXT NULL |
| expires_at | TIMESTAMPTZ |
| revoked_at | TIMESTAMPTZ NULL |
| created_at | TIMESTAMPTZ |

Session tokens must be stored as hashes rather than plaintext bearer secrets.

Track last activity and absolute expiry for the PWA session. Refresh replaces the hash atomically without extending the absolute lifetime. Recovery revokes the customer’s sessions and outstanding transaction challenges and proofs.

## 8.4 identity.devices

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| device_name | TEXT |
| device_fingerprint_hash | TEXT NULL |
| last_seen_at | TIMESTAMPTZ |
| trusted_at | TIMESTAMPTZ NULL |
| created_at | TIMESTAMPTZ |

Device fingerprinting must follow applicable privacy requirements.

## 8.5 identity.verification_challenges

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| challenge_type | TEXT |
| destination | TEXT |
| code_hash | TEXT |
| expires_at | TIMESTAMPTZ |
| consumed_at | TIMESTAMPTZ NULL |
| attempt_count | INTEGER |
| created_at | TIMESTAMPTZ |

Verification codes must not be stored in plaintext.

For a financial challenge also persist the authenticated session, operation type and immutable, versioned binding (or canonical digest) from 06-SECURITY-COMPLIANCE.md SEC-TXN-002. Keep challenge completion separate from proof consumption: store only the unique proof hash, proof expiry, revocation and accepted operation ID. Atomically consume the unexpired proof once with operation acceptance, limit use and the request-idempotency record. Successful identical replay reads the recorded result.

---

# 9. CUSTOMER AND USERNAME TABLES

## 9.1 customers.profiles

| Column | Type |
|---|---|
| user_id | UUID PK/FK |
| first_name | TEXT |
| last_name | TEXT |
| date_of_birth | DATE NULL |
| country_code | CHAR(2) |
| residential_address_id | UUID NULL |
| preferred_currency | TEXT |
| created_at | TIMESTAMPTZ |
| updated_at | TIMESTAMPTZ |

## 9.2 customers.usernames

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| username | TEXT |
| normalized_username | TEXT |
| is_active | BOOLEAN |
| created_at | TIMESTAMPTZ |
| retired_at | TIMESTAMPTZ NULL |

Constraints:

- Unique active normalized username.
- One active username per customer.
- Reserved username restrictions.
- Username format validation.

Username resolution must use the internal user ID.

The username must not be treated as a blockchain address.

## 9.3 customers.preferences

| Column | Type |
|---|---|
| user_id | UUID PK/FK |
| phone_discoverable | BOOLEAN |
| email_notifications | BOOLEAN |
| push_notifications | BOOLEAN |
| marketing_consent | BOOLEAN |
| updated_at | TIMESTAMPTZ |

Phone discovery must be opt-in unless a different lawful and approved policy is established.

## 9.4 customers.addresses

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| address_type | TEXT |
| line_1 | TEXT |
| line_2 | TEXT NULL |
| city | TEXT |
| state_region | TEXT |
| postal_code | TEXT NULL |
| country_code | CHAR(2) |
| created_at | TIMESTAMPTZ |
| updated_at | TIMESTAMPTZ |

Sensitive address data must be protected.

---

# 10. KYC AND COMPLIANCE TABLES

## 10.1 compliance.kyc_cases

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| status | TEXT |
| risk_tier | TEXT |
| submitted_at | TIMESTAMPTZ NULL |
| reviewed_at | TIMESTAMPTZ NULL |
| reviewer_id | UUID NULL |
| rejection_reason_code | TEXT NULL |
| created_at | TIMESTAMPTZ |
| updated_at | TIMESTAMPTZ |

Suggested statuses:

- DRAFT.
- SUBMITTED.
- PENDING_PROVIDER.
- MANUAL_REVIEW.
- INFORMATION_REQUESTED.
- APPROVED.
- REJECTED.
- EXPIRED.

Approve, reject and request-information actions must check reviewer permission and the expected case state. Commit reviewer, reason, prior/new status and an append-only audit event with the case update. INFORMATION_REQUESTED permits resubmission; only APPROVED satisfies KYC approval.

## 10.2 compliance.kyc_documents

| Column | Type |
|---|---|
| id | UUID PK |
| kyc_case_id | UUID FK |
| document_type | TEXT |
| storage_reference | TEXT |
| verification_status | TEXT |
| created_at | TIMESTAMPTZ |

Documents should be stored in protected object storage.

The database should store secure references rather than large raw document contents.

## 10.3 compliance.verification_results

| Column | Type |
|---|---|
| id | UUID PK |
| kyc_case_id | UUID FK |
| provider_id | UUID FK |
| external_reference | TEXT |
| result_status | TEXT |
| result_summary | JSONB |
| verified_at | TIMESTAMPTZ NULL |
| created_at | TIMESTAMPTZ |

Provider responses must be minimized and redacted where necessary.

## 10.4 compliance.risk_cases

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| transaction_id | UUID NULL |
| case_type | TEXT |
| severity | TEXT |
| status | TEXT |
| assigned_to | UUID NULL |
| created_at | TIMESTAMPTZ |
| resolved_at | TIMESTAMPTZ NULL |

## 10.5 compliance.account_restrictions

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| restriction_type | TEXT |
| reason_code | TEXT |
| active | BOOLEAN |
| created_by | UUID NULL |
| created_at | TIMESTAMPTZ |
| lifted_at | TIMESTAMPTZ NULL |

## 10.6 compliance.transaction_limits

| Column | Type |
|---|---|
| id | UUID PK |
| scope_type | TEXT |
| scope_id | UUID NULL |
| transaction_type | TEXT |
| asset_id | UUID NULL |
| period | TEXT |
| limit_amount | NUMERIC(38,0) |
| enabled | BOOLEAN |
| created_at | TIMESTAMPTZ |
| updated_at | TIMESTAMPTZ |

Limits must be interpreted using the relevant asset precision.

Store each operation’s limit usage under UNIQUE (limit rule, scope, window, transaction_id). For each applicable shared rule, lock the same customer/window bucket in a consistent order across withdrawals and bank payments, then check and consume capacity in the transaction accepting the operation. Use the configured unit and valuation rule for cross-asset limits. Count pending and uncertain amounts; completion keeps the charge. Release unused capacity once on definitive non-execution. Preserve the original period’s usage for executed and later reversed/refunded amounts. Approval of thresholds and windows belongs to Risk.

---

# 11. ASSET AND BLOCKCHAIN NETWORK TABLES

## 11.1 wallets.assets

| Column | Type |
|---|---|
| id | UUID PK |
| symbol | TEXT |
| name | TEXT |
| asset_type | TEXT |
| accounting_precision | SMALLINT |
| enabled | BOOLEAN |
| created_at | TIMESTAMPTZ |

Initial assets:

- USDC.
- USDT.
- NGN for settlement and accounting purposes.

NGN must not automatically be exposed as a customer stored-value wallet.

## 11.2 wallets.networks

| Column | Type |
|---|---|
| id | UUID PK |
| code | TEXT |
| name | TEXT |
| chain_family | TEXT |
| chain_identifier | TEXT NULL |
| enabled | BOOLEAN |
| created_at | TIMESTAMPTZ |

Examples of possible networks:

- Ethereum.
- Base.
- Polygon.
- Solana.

Actual launch support requires provider confirmation.

## 11.3 wallets.asset_networks

| Column | Type |
|---|---|
| id | UUID PK |
| asset_id | UUID FK |
| network_id | UUID FK |
| token_contract | TEXT NULL |
| token_decimals | SMALLINT |
| deposit_enabled | BOOLEAN |
| withdrawal_enabled | BOOLEAN |
| created_at | TIMESTAMPTZ |

Constraints:

- Unique supported asset/network/token identity.
- Valid precision.
- Controlled activation.

A token's network precision must not be assumed to equal the customer ledger accounting precision.

---

# 12. WALLET AND ADDRESS TABLES

## 12.1 wallets.wallet_profiles

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| asset_id | UUID FK |
| status | TEXT |
| provisioning_status | TEXT |
| created_at | TIMESTAMPTZ |
| updated_at | TIMESTAMPTZ |

Constraints:

- Unique user/asset wallet profile where the product permits one profile per asset.
- Controlled status values.

## 12.2 wallets.wallet_addresses

| Column | Type |
|---|---|
| id | UUID PK |
| wallet_profile_id | UUID FK |
| asset_network_id | UUID FK |
| provider_id | UUID FK |
| address | TEXT |
| memo_tag | TEXT NULL |
| external_wallet_reference | TEXT NULL |
| status | TEXT |
| created_at | TIMESTAMPTZ |
| retired_at | TIMESTAMPTZ NULL |

A wallet address may be shared by multiple supported tokens on the same network depending on provider design.

The schema must not incorrectly assume one blockchain address can belong to only one asset.

Address assignment uniqueness must be enforced using the actual custody model.

## 12.3 wallets.wallet_provisioning_requests

| Column | Type |
|---|---|
| id | UUID PK |
| wallet_profile_id | UUID FK |
| provider_id | UUID FK |
| request_reference | TEXT |
| status | TEXT |
| attempt_count | INTEGER |
| last_error_code | TEXT NULL |
| created_at | TIMESTAMPTZ |
| updated_at | TIMESTAMPTZ |

## 12.4 Phase 6 Implementation (migration 202611261401)

**Registries**
- `wallets.assets` restricts asset types to `STABLECOIN` and `FIAT`, adds an approval reference and suspension, and requires ledger chart accounts before an asset is enabled. USDC and USDT (6 decimals) and NGN (2) are seeded.
- `wallets.networks` supports the EVM and Solana families, each with a validated chain identifier; a family and identifier pair is unique. It adds deposit and withdrawal flags, a memo requirement, a confirmation rule and an evidence reference.
- `wallets.asset_networks` adds verification and lifecycle states. A contract is validated and canonicalized per family, each contract maps to one asset per network, and at most one enabled mapping exists per asset and network.
- The identities of assets, networks, mappings, providers and capabilities never change.
- Every registry insert and update is kept in `wallets.registry_changes`.

**Provider register**
- `integrations.providers` adds a lifecycle status; an enabled provider must be `APPROVED`.
- `integrations.provider_capabilities` adds a verification stage, evidence, routing priority and operational health. Production approval is tied to the `PRODUCTION_ENABLED` stage.

**Wallet profiles and history**
- `wallets.wallet_profiles` provisioning status is `PENDING` until the ledger accounts exist, then `COMPLETE`.
- Status transitions are enforced, `CLOSED` is final and profiles are never deleted.
- `wallets.wallet_status_history` records each change with its actor and database login.

**Provisioning requests and addresses**
- `wallets.wallet_provisioning_requests` adds the asset/network, the provider's reference and a state machine: `PENDING`, `SUBMITTED`, `UNKNOWN`, then `PROVISIONED`, `FAILED` or `UNSUPPORTED`. At most one open request exists per wallet and asset/network.
- `wallets.wallet_addresses` adds the network, verification state, evidence, activation and retirement.
  - An address is shown only when verified, and is retired rather than replaced.
  - One on-chain address (with its memo) on a network belongs to one customer and one provider, and may serve that customer's several tokens.
- Wallet reads, creation and requests run through SECURITY DEFINER functions owned by `deypay_wallet_owner`. Balances come from the ledger, never from a wallet column.

---

# 13. FINANCIAL LEDGER TABLES

This section defines the structural foundation of the DEYPAY ledger.

See 05-LEDGER-ACCOUNTING.md for posting rules and the mappings pending Finance approval.

## 13.1 ledger.accounts

| Column | Type |
|---|---|
| id | UUID PK |
| account_code | TEXT UNIQUE |
| owner_type | TEXT |
| owner_id | UUID NULL |
| asset_id | UUID FK |
| account_type | TEXT |
| normal_balance | TEXT |
| status | TEXT |
| created_at | TIMESTAMPTZ |

Examples of account types:

- ASSET.
- LIABILITY.
- REVENUE.
- EXPENSE.
- EQUITY.
- CLEARING.
- SUSPENSE.

The account type and normal balance must follow the approved chart of accounts.

## 13.2 ledger.journals

| Column | Type |
|---|---|
| id | UUID PK |
| journal_reference | TEXT UNIQUE |
| accounting_event_type | TEXT |
| source_transaction_id | UUID NULL |
| status | TEXT |
| reversal_of_journal_id | UUID NULL |
| posted_at | TIMESTAMPTZ NULL |
| created_at | TIMESTAMPTZ |

Suggested statuses:

- DRAFT.
- POSTED.
- VOIDED.

A posted journal must never be edited or deleted.

## 13.3 ledger.entries

| Column | Type |
|---|---|
| id | UUID PK |
| journal_id | UUID FK |
| ledger_account_id | UUID FK |
| asset_id | UUID FK |
| direction | TEXT |
| amount_minor | NUMERIC(38,0) |
| created_at | TIMESTAMPTZ |

Constraints:

- amount_minor must be greater than zero.
- direction must be DEBIT or CREDIT.
- Entry asset must equal its ledger account's asset.
- Posted journal entries must be immutable.

## 13.4 ledger.balance_projections

| Column | Type |
|---|---|
| ledger_account_id | UUID PK/FK |
| balance_minor | NUMERIC(38,0) |
| version | BIGINT |
| updated_at | TIMESTAMPTZ |

Balance projections must be reconstructable from posted ledger entries.

The ledger is authoritative; projections are performance optimizations.

## 13.5 ledger.accounting_events

| Column | Type |
|---|---|
| id | UUID PK |
| event_type | TEXT |
| economic_event_key | TEXT |
| posting_purpose | TEXT |
| source_transaction_id | UUID NULL |
| journal_id | UUID FK |
| created_at | TIMESTAMPTZ |

Accounting events must support traceability between business operations and posted journals.

Persist a stable posting key with a UNIQUE constraint on (economic_event_key, posting_purpose). Derive it from the verified business fact and leg, independently of delivery channel or journal reference. Commit the claim, journal and state transition together. A repeated claim returns the recorded result. If a posting has multiple asset legs, balance each asset in that posting or key each distinct journal leg explicitly.

## 13.6 Ledger Posting Integrity

The database must enforce the following:

1. Every posted journal has at least two entries.
2. Every posted journal balances by asset.
3. Every entry references a valid ledger account.
4. Every entry asset matches its account asset.
5. Posted entries cannot be modified.
6. Posted journals cannot be deleted.
7. Journal references are unique.
8. Reversals reference the original journal.
9. Posting is atomic.
10. Financial projections update consistently.

A simple row-level CHECK constraint is insufficient to enforce balance across multiple journal entries.

The implementation must use a controlled posting function, appropriate locking, database privileges and/or deferred database constraints.

The exact enforcement design will be specified and tested in the Ledger and Accounting Design.

---

# 14. BALANCE RESERVATION TABLES

## 14.1 ledger.reservations

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| ledger_account_id | UUID FK |
| transaction_id | UUID FK |
| amount_minor | NUMERIC(38,0) |
| status | TEXT |
| created_at | TIMESTAMPTZ |
| updated_at | TIMESTAMPTZ |
| released_at | TIMESTAMPTZ NULL |
| settled_at | TIMESTAMPTZ NULL |

Statuses:

- ACTIVE.
- SETTLED.
- RELEASED.
- CANCELLED.

## 14.2 Reservation Rules

A reservation must:

- Reference one financial operation.
- Use a valid ledger account.
- Use the account's asset.
- Have a positive amount.
- Prevent the reserved funds from being spent again.
- Be created atomically with the relevant transaction state.
- Be released only when safe.
- Be settled according to the actual financial outcome.

## 14.3 Available Balance

Use dedicated available and reserved customer liability accounts as specified in 05-LEDGER-ACCOUNTING.md §§10–11. A reservation journal moves value between them. The reservation row tracks the operation and changes in the same database transaction as its journal. Read available funds directly from the available liability account.

---

# 15. TRANSACTION TABLES

## 15.1 payments.transactions

This is the unified business transaction record.

| Column | Type |
|---|---|
| id | UUID PK |
| public_reference | TEXT UNIQUE |
| user_id | UUID FK |
| transaction_type | TEXT |
| status | TEXT |
| source_asset_id | UUID NULL |
| destination_asset_id | UUID NULL |
| source_amount_minor | NUMERIC(38,0) NULL |
| destination_amount_minor | NUMERIC(38,0) NULL |
| fee_amount_minor | NUMERIC(38,0) NULL |
| fee_asset_id | UUID NULL |
| description | TEXT NULL |
| created_at | TIMESTAMPTZ |
| updated_at | TIMESTAMPTZ |
| completed_at | TIMESTAMPTZ NULL |

## 15.2 Transaction Categories

- DEPOSIT.
- WITHDRAWAL.
- INTERNAL_TRANSFER.
- BANK_PAYMENT.
- BANK_WITHDRAWAL.
- FUNDING_ORDER.
- CONVERSION.
- ADJUSTMENT.
- REVERSAL.

## 15.3 Transaction Rules

A transaction record represents the business operation.

The ledger represents its financial accounting.

A transaction's displayed status must not be used as the sole proof that accounting has been posted.

The system must validate the relationship between workflow state and ledger state.

For FUNDING_ORDER, persist payments.funding_orders with order ID, unique transaction FK, customer and quote FKs, expected NGN amount (minor units), destination asset, expiry and workflow state. Link the provider-scoped collection reference and its unique confirmation evidence, conversion or delivery operation, unique stablecoin credit event and any failure reason or reconciliation case. Preserve unmatched collections for investigation. Enforce uniqueness on the underlying collection event across orders.

## 15.4 payments.transaction_status_history

| Column | Type |
|---|---|
| id | UUID PK |
| transaction_id | UUID FK |
| previous_status | TEXT NULL |
| new_status | TEXT |
| reason_code | TEXT NULL |
| actor_type | TEXT |
| actor_id | UUID NULL |
| created_at | TIMESTAMPTZ |

Status transitions must be append-only.

---

# 16. STABLECOIN DEPOSIT TABLES

## 16.1 payments.deposits

| Column | Type |
|---|---|
| id | UUID PK |
| transaction_id | UUID FK |
| user_id | UUID FK |
| wallet_address_id | UUID FK |
| asset_network_id | UUID FK |
| provider_id | UUID FK |
| blockchain_tx_hash | TEXT |
| transfer_index | TEXT NULL |
| amount_base_units | NUMERIC(38,0) |
| confirmation_count | INTEGER |
| status | TEXT |
| detected_at | TIMESTAMPTZ |
| credited_at | TIMESTAMPTZ NULL |

## 16.2 Deposit Uniqueness

The deposit identity must account for:

- Network.
- Transaction hash.
- Token transfer event or instruction.
- Asset.
- Recipient address.

A transaction hash alone may contain multiple token transfers.

The exact uniqueness constraint must be selected for each supported chain family.

Enforce UNIQUE on the normalized, non-null chain transfer identity (including the instruction or log discriminator when applicable). Use this identity for the credit posting even when multiple providers observe the transfer.

## 16.3 Deposit Crediting

A deposit must have a unique accounting credit event.

The system must prevent multiple ledger credits for the same verified deposit.

---

# 17. STABLECOIN WITHDRAWAL TABLES

## 17.1 payments.withdrawals

| Column | Type |
|---|---|
| id | UUID PK |
| transaction_id | UUID FK |
| user_id | UUID FK |
| wallet_profile_id | UUID FK |
| asset_network_id | UUID FK |
| destination_address | TEXT |
| destination_memo | TEXT NULL |
| amount_minor | NUMERIC(38,0) |
| fee_minor | NUMERIC(38,0) |
| reservation_id | UUID FK |
| provider_operation_id | UUID NULL |
| blockchain_tx_hash | TEXT NULL |
| status | TEXT |
| created_at | TIMESTAMPTZ |
| completed_at | TIMESTAMPTZ NULL |

## 17.2 Withdrawal Rules

- Destination address must be validated.
- Asset/network compatibility must be enforced.
- Reservation must exist before provider execution.
- Provider operation references must be retained.
- Unknown provider outcomes must remain reconcilable.
- Completion must be based on configured settlement criteria.

---

# 18. INTERNAL TRANSFER TABLES

## 18.1 payments.internal_transfers

| Column | Type |
|---|---|
| id | UUID PK |
| transaction_id | UUID FK |
| sender_user_id | UUID FK |
| recipient_user_id | UUID FK |
| asset_id | UUID FK |
| amount_minor | NUMERIC(38,0) |
| fee_minor | NUMERIC(38,0) |
| status | TEXT |
| ledger_journal_id | UUID NULL |
| created_at | TIMESTAMPTZ |
| completed_at | TIMESTAMPTZ NULL |

## 18.2 Constraints

- Sender must exist.
- Recipient must exist.
- Sender and recipient must differ.
- Amount must be positive.
- Asset must be supported.
- One completed transfer must map to the required unique accounting posting.

## 18.3 Atomicity

The sender debit and recipient credit must occur atomically.

The transaction must not enter COMPLETED unless the corresponding ledger posting is committed.

Scope internal-transfer list, detail and receipt reads by sender_user_id or recipient_user_id on the same transfer row. The unified transaction's initiating user alone is insufficient. Project direction relative to the viewer; neither participant receives access to the other’s account history.

## 18.4 Concurrency

Concurrent transfers must be serialized or otherwise protected against overspending.

---

# 19. FX QUOTE AND CONVERSION TABLES

## 19.1 fx.quotes

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| provider_id | UUID FK |
| external_quote_reference | TEXT NULL |
| source_asset_id | UUID FK |
| destination_asset_id | UUID FK |
| source_amount_minor | NUMERIC(38,0) |
| destination_amount_minor | NUMERIC(38,0) |
| exchange_rate | NUMERIC(38,18) |
| fee_amount_minor | NUMERIC(38,0) |
| fee_asset_id | UUID FK |
| expires_at | TIMESTAMPTZ |
| status | TEXT |
| created_at | TIMESTAMPTZ |

The precision of the exchange-rate field must be reviewed against the supported currency corridors.

Persist immutable execution terms with each executable quote, including customer, operation, verified destination or recipient, applicable network, amounts, fees and expiry. A changed destination or price requires fresh confirmation. Enforce one accepted execution per executable quote.

## 19.2 fx.conversions

| Column | Type |
|---|---|
| id | UUID PK |
| transaction_id | UUID FK |
| quote_id | UUID FK |
| provider_operation_id | UUID NULL |
| status | TEXT |
| executed_at | TIMESTAMPTZ NULL |
| settled_at | TIMESTAMPTZ NULL |
| created_at | TIMESTAMPTZ |

## 19.3 Conversion Rules

- Quote must be valid at acceptance.
- Execution must be uniquely identified.
- Exact executed amounts must be recorded.
- Fees must be accounted for.
- Conversion settlement must be reconciled.

---

# 20. NIGERIAN BANK AND BENEFICIARY TABLES

## 20.1 payments.banks

| Column | Type |
|---|---|
| id | UUID PK |
| bank_code | TEXT |
| bank_name | TEXT |
| country_code | CHAR(2) |
| enabled | BOOLEAN |
| provider_id | UUID NULL |
| updated_at | TIMESTAMPTZ |

## 20.2 payments.bank_beneficiaries

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| bank_id | UUID FK |
| account_number_encrypted | BYTEA |
| account_number_fingerprint | TEXT |
| account_number_last4 | TEXT |
| resolved_account_name | TEXT |
| verification_status | TEXT |
| provider_reference | TEXT NULL |
| created_at | TIMESTAMPTZ |
| updated_at | TIMESTAMPTZ |

Store the owner's editable display_label on bank beneficiaries. Persist saved internal recipients separately in payments.internal_beneficiaries (id, owner_user_id FK, recipient_user_id FK, display_label, timestamps; UNIQUE owner/recipient). Both beneficiary types are owner-scoped. A rename changes only the label and preserves the verified destination and historical transaction evidence.

## 20.3 Bank Account Protection

Full bank account numbers must be encrypted.

The system may store a keyed fingerprint for duplicate detection.

A plain cryptographic hash of a short account number is not sufficient protection against enumeration.

## 20.4 Beneficiary Verification

The resolved account name must originate from a trusted bank account resolution provider.

Verification results must have an appropriate validity period.

---

# 21. PAY ANYONE TABLES

## 21.1 payments.bank_payments

| Column | Type |
|---|---|
| id | UUID PK |
| transaction_id | UUID FK |
| sender_user_id | UUID FK |
| beneficiary_id | UUID FK |
| funding_asset_id | UUID FK |
| funding_amount_minor | NUMERIC(38,0) |
| payout_currency | TEXT |
| payout_amount_minor | NUMERIC(38,0) |
| quote_id | UUID FK |
| reservation_id | UUID FK |
| conversion_id | UUID NULL |
| payout_provider_operation_id | UUID NULL |
| status | TEXT |
| created_at | TIMESTAMPTZ |
| completed_at | TIMESTAMPTZ NULL |

## 21.2 Payment Processing

A bank payment must maintain durable references to:

- Sender.
- Beneficiary.
- Quote.
- Reservation.
- Conversion.
- Provider payout.
- Transaction status.
- Financial journal entries.

## 21.3 Payout Outcome

Use the bank workflow and exception states in 05-LEDGER-ACCOUNTING.md §§25.4–25.5. A payout leg can fail while the parent operation still owes NGN.

The payment must not be marked failed merely because the provider timed out.

Persist conversion and payout states separately. Link the bank payment to its reservation, conversion journal and outstanding NGN obligation in the ledger. Record recovery actions with unique action ID, transaction, amount, asset, disposition, evidence and resulting journal reference. A confirmed failed payout leaves the obligation open until an approved replacement payment or refund settles it.

## 21.4 Settlement Records

Separate settlement records may be required to represent:

- NGN liquidity.
- Provider settlement balance.
- Payout funding.
- Payout confirmation.
- Settlement reconciliation.

The exact settlement tables will be finalized after provider contracts and accounting workflows are confirmed.

---

# 22. PROVIDER INTEGRATION TABLES

## 22.1 integrations.providers

| Column | Type |
|---|---|
| id | UUID PK |
| provider_code | TEXT UNIQUE |
| provider_name | TEXT |
| provider_type | TEXT |
| enabled | BOOLEAN |
| created_at | TIMESTAMPTZ |

Examples:

- NUVION.
- NGN_PAYOUT_PROVIDER.
- KYC_PROVIDER.
- AML_PROVIDER.

## 22.2 integrations.provider_accounts

| Column | Type |
|---|---|
| id | UUID PK |
| provider_id | UUID FK |
| internal_owner_type | TEXT |
| internal_owner_id | UUID |
| external_account_reference | TEXT |
| status | TEXT |
| created_at | TIMESTAMPTZ |

## 22.3 integrations.provider_operations

| Column | Type |
|---|---|
| id | UUID PK |
| provider_id | UUID FK |
| provider_account_scope | TEXT |
| transaction_id | UUID NULL |
| operation_type | TEXT |
| leg | TEXT |
| authorized_attempt | INTEGER |
| external_reference | TEXT NULL |
| idempotency_reference | TEXT |
| status | TEXT |
| last_error_code | TEXT NULL |
| created_at | TIMESTAMPTZ |
| updated_at | TIMESTAMPTZ |

Use idempotency_reference as the DEYPAY submission reference even when the provider has no replay support. For financial dispatches require a transaction ID and enforce UNIQUE (provider_id, provider_account_scope, idempotency_reference) and UNIQUE (transaction_id, leg, authorized_attempt). Persist the reference before network I/O. Record acceptance, definitive non-execution or uncertainty against it. Claim each verified external event under a unique provider/account + economic reference + leg identity, shared by webhook, polling and statement import. Preserve each observation as evidence. Chain transfers use §16.2 across providers.

## 22.4 Provider Independence

DEYPAY business entities must reference internal provider records.

External identifiers must be treated as opaque values.

## 22.5 Provider Capabilities

A dedicated capability table should track:

- Provider.
- Service.
- Asset.
- Network.
- Currency corridor.
- Country.
- Environment.
- Availability.
- Production approval.

This prevents the application from assuming that every provider supports every operation.

---

# 23. WEBHOOK AND EVENT PROCESSING TABLES

## 23.1 integrations.webhook_events

| Column | Type |
|---|---|
| id | UUID PK |
| provider_id | UUID FK |
| external_event_id | TEXT NULL |
| event_type | TEXT |
| payload_reference | TEXT NULL |
| payload_hash | TEXT |
| signature_verified | BOOLEAN |
| processing_status | TEXT |
| received_at | TIMESTAMPTZ |
| processed_at | TIMESTAMPTZ NULL |

## 23.2 Webhook Deduplication

Use provider event IDs when reliable.

Where unavailable, use an approved event identity strategy.

Payload hashes alone may not be sufficient for all event types.

## 23.3 integrations.outbox_events

MVP registration writes one `EMAIL_VERIFICATION_REQUESTED` intent in the signup transaction. The database rejects new `PHONE_VERIFICATION_REQUESTED` rows. Historical phone intents retain their original state, and the email dispatcher ignores them.

| Column | Type |
|---|---|
| id | UUID PK |
| aggregate_type | TEXT |
| aggregate_id | UUID |
| event_type | TEXT |
| payload | JSONB |
| status | TEXT |
| created_at | TIMESTAMPTZ |
| published_at | TIMESTAMPTZ NULL |

Outbox events must be inserted in the same database transaction as the associated business state change.

## 23.4 integrations.jobs

| Column | Type |
|---|---|
| id | UUID PK |
| job_type | TEXT |
| payload | JSONB |
| status | TEXT |
| attempts | INTEGER |
| available_at | TIMESTAMPTZ |
| locked_until | TIMESTAMPTZ NULL |
| last_error | TEXT NULL |
| created_at | TIMESTAMPTZ |

The job processor must support:

- Durable storage.
- Safe claiming.
- Retry limits.
- Lease expiry.
- Recovery.
- Idempotent execution.

---

# 24. NOTIFICATIONS AND PAYMENT LINKS

## 24.1 operations.notifications

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| notification_type | TEXT |
| title | TEXT |
| body | TEXT |
| related_transaction_id | UUID NULL |
| read_at | TIMESTAMPTZ NULL |
| created_at | TIMESTAMPTZ |

## 24.2 operations.notification_deliveries

| Column | Type |
|---|---|
| id | UUID PK |
| notification_id | UUID FK |
| channel | TEXT |
| provider_reference | TEXT NULL |
| status | TEXT |
| attempts | INTEGER |
| created_at | TIMESTAMPTZ |
| delivered_at | TIMESTAMPTZ NULL |

## 24.3 payments.payment_links

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| public_token_hash | TEXT |
| asset_id | UUID NULL |
| amount_minor | NUMERIC(38,0) NULL |
| status | TEXT |
| expires_at | TIMESTAMPTZ NULL |
| created_at | TIMESTAMPTZ |

Payment link tokens must be unpredictable and securely generated.

---

# 25. ADMINISTRATION AND AUDIT TABLES

## 25.1 audit.admin_roles

| Column | Type |
|---|---|
| id | UUID PK |
| role_code | TEXT UNIQUE |
| description | TEXT |
| created_at | TIMESTAMPTZ |

## 25.2 audit.admin_permissions

| Column | Type |
|---|---|
| id | UUID PK |
| permission_code | TEXT UNIQUE |
| description | TEXT |

## 25.3 audit.role_permissions

| Column | Type |
|---|---|
| role_id | UUID FK |
| permission_id | UUID FK |

Composite primary key:

(role_id, permission_id)

## 25.4 audit.admin_assignments

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| role_id | UUID FK |
| assigned_at | TIMESTAMPTZ |
| revoked_at | TIMESTAMPTZ NULL |

## 25.5 audit.audit_events

| Column | Type |
|---|---|
| id | UUID PK |
| actor_type | TEXT |
| actor_id | UUID NULL |
| action | TEXT |
| resource_type | TEXT |
| resource_id | UUID NULL |
| reason | TEXT NULL |
| correlation_id | UUID NULL |
| metadata | JSONB |
| created_at | TIMESTAMPTZ |

Audit events must be append-only.

## 25.6 audit.approval_requests

| Column | Type |
|---|---|
| id | UUID PK |
| action_type | TEXT |
| target_reference | TEXT |
| requested_by | UUID FK |
| approved_by | UUID NULL |
| status | TEXT |
| reason | TEXT |
| created_at | TIMESTAMPTZ |
| decided_at | TIMESTAMPTZ NULL |

Maker-checker rules must prevent an administrator from approving their own restricted action.

---

# 26. RECONCILIATION TABLES

## 26.1 operations.reconciliation_runs

| Column | Type |
|---|---|
| id | UUID PK |
| provider_id | UUID FK |
| reconciliation_type | TEXT |
| period_start | TIMESTAMPTZ |
| period_end | TIMESTAMPTZ |
| status | TEXT |
| started_at | TIMESTAMPTZ |
| completed_at | TIMESTAMPTZ NULL |

## 26.2 operations.reconciliation_items

| Column | Type |
|---|---|
| id | UUID PK |
| reconciliation_run_id | UUID FK |
| internal_transaction_id | UUID NULL |
| provider_operation_id | UUID NULL |
| external_evidence_reference | TEXT NULL |
| evidence_observed_at | TIMESTAMPTZ NULL |
| observed_asset_id | UUID NULL |
| observed_amount_minor | NUMERIC(38,0) NULL |
| match_status | TEXT |
| discrepancy_type | TEXT NULL |
| created_at | TIMESTAMPTZ |

Each match or unmatched external item must retain the source provider/account, immutable statement line, chain transfer or status evidence reference, observed timestamp, amount and asset. Preserve the source record or a retrievable version with integrity hash. Retain the evidence even if no internal transaction exists.

## 26.3 operations.reconciliation_cases

| Column | Type |
|---|---|
| id | UUID PK |
| reconciliation_item_id | UUID FK |
| severity | TEXT |
| status | TEXT |
| assigned_to | UUID NULL |
| resolution_notes | TEXT NULL |
| created_at | TIMESTAMPTZ |
| resolved_at | TIMESTAMPTZ NULL |

## 26.4 Reconciliation Requirements

The system must identify:

- Missing transactions.
- Duplicate transactions.
- Amount mismatches.
- Unexpected provider balances.
- Fee mismatches.
- Missing settlement confirmation.
- Stuck transactions.

Critical discrepancies must generate operational alerts.

---

# 27. SUPPORT AND DISPUTE TABLES

## 27.1 operations.support_tickets

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| transaction_id | UUID NULL |
| subject | TEXT |
| description | TEXT |
| status | TEXT |
| priority | TEXT |
| assigned_to | UUID NULL |
| created_at | TIMESTAMPTZ |
| updated_at | TIMESTAMPTZ |

## 27.2 operations.support_messages

| Column | Type |
|---|---|
| id | UUID PK |
| ticket_id | UUID FK |
| sender_id | UUID |
| sender_type | TEXT |
| message | TEXT |
| internal_only | BOOLEAN |
| created_at | TIMESTAMPTZ |

Support personnel must not receive unrestricted access to sensitive KYC documents or financial adjustment capabilities.

---

# 28. RELATIONSHIP AND FOREIGN KEY RULES

## 28.1 Customer Ownership

Every customer-owned financial record must reference a valid internal customer.

## 28.2 Wallet Ownership

Every wallet profile must reference a valid customer and asset.

## 28.3 Ledger Relationships

Every ledger entry must reference a valid journal and ledger account.

## 28.4 Transaction Relationships

Every specialized transaction must reference the unified transaction record.

## 28.5 Provider Relationships

Every provider operation must reference a valid provider.

## 28.6 Referential Deletion

Financial records must not use cascading deletion from customer accounts.

Customer closure must not delete required financial history.

## 28.7 Historical Integrity

Historical provider references, accounting records and transaction records must remain available for required reconciliation and retention periods.

---

# 29. FINANCIAL INTEGRITY CONSTRAINTS

The database must enforce the following invariants.

**FI-001:** Posted journals balance per asset.

**FI-002:** Posted entries are immutable.

**FI-003:** Ledger entry amounts are positive.

**FI-004:** Ledger entry asset matches account asset.

**FI-005:** Financial operation references are unique where required.

**FI-006:** Duplicate provider events do not create duplicate credits.

**FI-007:** Customer available balances cannot become negative through ordinary spending.

**FI-008:** Internal transfers post atomically.

**FI-009:** External payouts cannot be blindly retried after uncertain execution.

**FI-010:** Reservations cannot be released twice.

**FI-011:** Completed transactions have the required accounting evidence.

**FI-012:** Financial corrections use compensating entries.

**FI-013:** Cross-asset conversion accounting remains balanced within each asset.

**FI-014:** Every externally executed financial operation has a traceable provider reference.

**FI-015:** Financial reconciliation discrepancies remain auditable.

---

# 30. TRANSACTION ISOLATION AND CONCURRENCY

## 30.1 PostgreSQL Transactions

Financial operations must use explicit database transactions.

## 30.2 Isolation Strategy

The appropriate isolation level must be selected for each operation.

For balance-sensitive operations, row locking or serializable isolation may be necessary.

## 30.3 Lock Ordering

Operations affecting multiple accounts must use a consistent lock ordering strategy to reduce deadlocks.

## 30.4 Concurrent Spending

The system must prevent two simultaneous requests from spending the same funds.

## 30.5 Optimistic Concurrency

Version columns may be used for appropriate balance projections and workflow records.

## 30.6 Retry Safety

Database serialization failures may be retried only when the operation is safe to retry and the complete transaction is replayed correctly.

External financial execution must not be repeated merely because an internal database transaction was retried.

---

# 31. IDEMPOTENCY DESIGN

## 31.1 payments.idempotency_records

| Column | Type |
|---|---|
| id | UUID PK |
| user_id | UUID FK |
| operation_type | TEXT |
| idempotency_key | TEXT |
| request_hash | TEXT |
| transaction_id | UUID NULL |
| response_status | INTEGER NULL |
| response_body | JSONB NULL |
| status | TEXT |
| created_at | TIMESTAMPTZ |
| expires_at | TIMESTAMPTZ |

## 31.2 Unique Constraint

A suitable uniqueness scope is:

(user_id, operation_type, idempotency_key)

## 31.3 Behavior

Same key and same payload:

Return the existing operation or recorded result.

Same key and different payload:

Reject as a conflict.

## 31.4 Persistence

Financial idempotency must survive application restarts.

Short-lived in-memory deduplication is insufficient.

## 31.5 Retention

Idempotency retention must account for the financial operation lifecycle and provider retry behavior.

Deleting an idempotency record must not permit duplicate execution of a still-relevant financial operation.

---

# 32. INDEXING AND QUERY OPTIMIZATION

Recommended indexes include:

## Identity

- Unique normalized email.
- Verified phone lookup.
- Active sessions by user.
- Session expiry.

## Customers

- Unique active username.
- User profile lookup.

## Wallets

- Wallet profiles by user.
- Wallet addresses by network and address.
- Provider wallet references.

## Ledger

- Entries by journal.
- Entries by ledger account.
- Journals by source transaction.
- Reservations by transaction.
- Active reservations by account.

## Payments

- Transactions by user and creation time.
- Transactions by status.
- Transactions by public reference.
- Bank payments by provider operation.
- Deposits by chain-specific transfer identity.

## Integrations

- Provider operations by external reference.
- Webhook events by provider and event ID.
- Pending jobs by availability and status.

## Reconciliation

- Runs by provider and period.
- Open discrepancies by severity.
- Reconciliation items by transaction reference.

Indexes must be selected based on measured query patterns.

Unnecessary indexes increase write overhead and storage usage.

---

# 33. DATABASE SECURITY

## 33.1 Database Access

Production database access must be restricted.

## 33.2 Database Roles

Recommended roles:

- Application runtime role.
- Migration role.
- Read-only reporting role.
- Backup role.
- Restricted administrative role.

## 33.3 Least Privilege

The runtime application must not require unrestricted schema modification permissions.

## 33.4 Encryption

Database connections must use encrypted transport.

Sensitive storage must use encryption at rest.

## 33.5 Auditability

Sensitive administrative database operations must be logged where practical.

## 33.6 Production Access

Direct production database changes must be exceptional, approved and auditable.

Financial records must never be manually edited as an ordinary operational procedure.

---

# 34. SENSITIVE DATA PROTECTION

Sensitive data includes:

- Password hashes.
- Session tokens.
- Government identity information.
- KYC documents.
- Phone numbers.
- Residential addresses.
- Bank account numbers.
- Provider credentials.
- Security investigation data.

Controls must include:

- Encryption.
- Access restrictions.
- Data minimization.
- Retention limits.
- Redacted logging.
- Secure object storage.
- Approved key management.

Provider credentials must not be stored as plaintext application configuration in the database.

---

# 35. DATA RETENTION AND PRIVACY

## 35.1 Retention Categories

Different records may have different retention requirements.

Examples:

- Authentication sessions.
- Verification challenges.
- Customer profiles.
- KYC documents.
- Financial transactions.
- Ledger entries.
- Audit logs.
- Support tickets.

## 35.2 Financial Records

Financial records must be retained according to applicable legal and regulatory requirements.

## 35.3 Customer Closure

Account closure must not automatically erase records required for legal compliance or financial reconciliation.

## 35.4 Privacy Rights

The architecture must support applicable data subject rights, subject to lawful retention obligations.

---

# 36. BACKUP AND DISASTER RECOVERY

## 36.1 Backups

The database must support automated backups.

## 36.2 Point-in-Time Recovery

Production should support point-in-time recovery where available.

## 36.3 Recovery Targets

Initial engineering targets:

- Recovery Point Objective: 15 minutes or better.
- Recovery Time Objective: 4 hours or better.

Targets must be validated through recovery testing.

## 36.4 Backup Testing

Backups must be periodically restored into a controlled environment.

A backup that has never been successfully restored must not be assumed reliable.

## 36.5 Financial Recovery

Following a database recovery, the system must reconcile external provider operations that may have occurred after the restored database state.

This is critical because external financial transactions cannot necessarily be rolled back with the database.

Keep affected dispatch and spending paused. Reconcile from the restore point through cutover, reconstruct missing operations and postings once from retained evidence and independently verify balances and outstanding obligations before resuming.

---

# 37. MIGRATION STRATEGY

## 37.1 Migration Tool

Use SQLx migrations.

## 37.2 Version Control

Every migration must be committed to source control.

## 37.3 Migration Naming

Example:

`202611170001_create_identity_schema.sql`

`202611170002_create_customers_schema.sql`

`202611170003_create_wallets_schema.sql`

`202611170004_create_ledger_schema.sql`

## 37.4 Migration Order

Migrations must respect foreign key dependencies.

## 37.5 Production Safety

Destructive schema changes must be reviewed.

Large-table migrations must consider locking and downtime.

## 37.6 Rollback

Rollback procedures must be documented.

Not every migration can be safely reversed automatically.

---

# 38. SQLX INTEGRATION

## 38.1 SQLx Usage

The Rust backend will use SQLx for PostgreSQL access.

## 38.2 Repository Pattern

Database queries should be encapsulated in infrastructure repositories.

Application services must not scatter raw SQL across unrelated modules.

## 38.3 Compile-Time Query Validation

Use SQLx query validation where practical.

## 38.4 Database Transactions

Financial application services must use explicit SQLx transactions.

## 38.5 Connection Pooling

Configure pools according to deployment capacity.

## 38.6 Error Mapping

Database errors must be translated into typed application errors.

Sensitive database details must not be exposed to customers.

---

# 39. TESTING AND VALIDATION

## 39.1 Schema Tests

Validate:

- Table creation.
- Foreign keys.
- Unique constraints.
- Check constraints.
- Migration ordering.

## 39.2 Ledger Tests

Validate:

- Balanced postings.
- Immutability.
- Reversals.
- Concurrent spending.
- Reservation handling.

## 39.3 Idempotency Tests

Validate duplicate requests and conflicting payloads.

## 39.4 Provider Tests

Validate duplicate webhook handling and provider reference mapping.

## 39.5 Concurrency Tests

Simulate multiple simultaneous transfers and withdrawals.

## 39.6 Recovery Tests

Simulate process crashes, database rollbacks and worker restarts.

## 39.7 Performance Tests

Test common customer, transaction and reconciliation queries under expected load.

---

# 40. OPERATIONAL MONITORING

Monitor:

- Database availability.
- Connection pool saturation.
- Slow queries.
- Lock contention.
- Deadlocks.
- Transaction rollback rates.
- Replication lag.
- Storage usage.
- Backup status.
- Migration failures.
- Ledger reconciliation discrepancies.

Critical database and financial alerts must be routed to the appropriate operations personnel.

---

# 41. DATABASE IMPLEMENTATION SEQUENCE

The recommended implementation order is:

## Phase 1: Foundational Schemas

- Identity.
- Customers.
- Audit.
- Provider registry.

## Phase 2: Asset and Wallet Models

- Assets.
- Networks.
- Asset/network mappings.
- Wallet profiles.
- Wallet addresses.

## Phase 3: Financial Ledger

- Ledger accounts.
- Journals.
- Entries.
- Balance projections.
- Reservations.
- Posting constraints.

## Phase 4: Transaction Models

- Unified transactions.
- Transaction status history.
- Idempotency records.

## Phase 5: Payment Models

- Deposits.
- Withdrawals.
- Internal transfers.
- Bank beneficiaries.
- Bank payments.

## Phase 6: FX and Provider Models

- Quotes.
- Conversions.
- Provider operations.
- Webhook events.

## Phase 7: Background Processing

- Outbox.
- Durable jobs.
- Reconciliation.

## Phase 8: Operations

- Notifications.
- Support.
- Administrative approvals.
- Audit reporting.

---

# 42. DESIGN RISKS AND MITIGATIONS

## Risk 1: Incorrect Ledger Design

Mitigation: Complete the accounting specification before financial posting implementation.

## Risk 2: Duplicate Deposits

Mitigation: Chain-specific deposit uniqueness and idempotent accounting.

## Risk 3: Concurrent Overspending

Mitigation: Atomic transactions, locking and balance constraints.

## Risk 4: Provider Data Model Coupling

Mitigation: Separate external references from internal entities.

## Risk 5: Uncertain Payout Outcomes

Mitigation: Durable workflow state and reconciliation.

## Risk 6: Sensitive Data Exposure

Mitigation: Encryption, restricted access and data minimization.

## Risk 7: Schema Migration Errors

Mitigation: Version-controlled migrations and staging validation.

## Risk 8: Recovery Inconsistency

Mitigation: Reconcile external provider operations after database restoration.

---

# 43. OPEN DESIGN DECISIONS

The following require final engineering approval.

1. Exact ledger chart of accounts.
2. Final reservation account codes and posting controls for the selected dedicated-account model.
3. Final asset accounting precision.
4. Initial blockchain networks.
5. Chain-specific deposit identity rules.
6. Custody address assignment model.
7. Provider operation reference formats.
8. Nigerian bank payout settlement structure.
9. NGN liquidity accounting.
10. Final financial posting constraints.
11. Database hosting provider.
12. Encryption and key management implementation.
13. Retention periods.
14. Reconciliation tolerance rules.
15. Production database access procedures.

These decisions must be resolved before implementing the affected production financial workflows.

---

# 44. DATABASE ACCEPTANCE CRITERIA

The database design will be accepted when:

1. Every MVP business entity has a defined persistence model.
2. All critical relationships are documented.
3. Internal IDs are independent of provider IDs.
4. Asset and network identities are explicit.
5. Financial amounts use exact representations.
6. Ledger journals can be enforced as balanced and immutable.
7. Internal transfers can be posted atomically.
8. Concurrent spending can be prevented.
9. Financial idempotency can be enforced.
10. Deposits can be uniquely identified.
11. Withdrawals can be traced to provider execution.
12. Bank payments can be reconciled.
13. Sensitive customer information can be protected.
14. Administrative actions can be audited.
15. Migrations can be applied consistently.
16. The database can be backed up and restored.
17. Automated tests validate financial integrity.
18. The schema supports provider replacement without rebuilding core business records.

---

# 45. FINAL DATABASE ARCHITECTURE DEFINITION

DEYPAY will use PostgreSQL as its authoritative relational database.

The database will maintain separate logical domains for identity, customers, compliance, wallets, financial accounting, payments, foreign exchange, provider integrations, administration and reconciliation.

All major business entities will use DEYPAY-generated identifiers.

USDC and USDT will be represented as supported customer wallet assets, while NGN will be modeled for conversion, settlement and accounting without automatically creating a general-purpose customer NGN wallet.

Customer wallet balances will be derived from a double-entry financial ledger.

Financial operations will use exact amount representations, database transactions, appropriate concurrency controls, immutable journals and idempotency.

External provider operations will be represented through independent provider reference mappings and durable workflow records.

Nuvion will be integrated without controlling DEYPAY's internal data model.

Nigerian bank payments will be represented as multi-step financial workflows involving customer authorization, balance reservation, conversion, external payout execution, settlement confirmation and reconciliation.

The database will support administrative oversight, compliance controls, customer support, audit trails and operational reporting.

The final physical implementation must be validated against the approved ledger accounting model, provider contracts and applicable regulatory requirements.

**FINAL DATABASE PRINCIPLE**

The database must never merely record what the application claims happened.

It must preserve sufficient evidence to establish what was requested, authorized, reserved, executed, settled, posted and reconciled.

Every customer balance must be explainable.

Every financial movement must be traceable.

Every external payment must be recoverable.

Every posted accounting event must be auditable.

---

**END OF DOCUMENT**

**DEYPAY DATABASE DESIGN SPECIFICATION**

**Document ID:** DP-DB-003  
**Version:** 1.0  
**Status:** Proposed Engineering Baseline
