# DEYPAY (DP)
# FINANCIAL LEDGER AND ACCOUNTING DESIGN SPECIFICATION
## Double-Entry Accounting, Customer Balances, Stablecoin Custody, Payment Settlement and Financial Integrity

**Document ID:** DP-LEDGER-005  
**Version:** 1.0  
**Status:** Proposed Engineering Baseline — Subject to Accounting, Legal and Provider Validation  
**Product:** DEYPAY  
**Initial Market:** Nigeria  
**Backend:** Rust / Axum / SQLx  
**Database:** PostgreSQL  
**Architecture:** Modular Monolith  
**Initial Customer Assets:** USDC and USDT  
**Settlement Currency:** NGN  
**Preferred Infrastructure Candidate:** Nuvion  
**Date:** 17 November 2026  
**Classification:** Confidential — Internal Engineering and Financial Operations

**Related Documents**

- DP-001 — Functional Requirements Specification
- DP-002 — System Architecture Document
- DP-003 — Database Design Specification
- DP-004 — API Specification and Contract Design

---

# DOCUMENT CONTROL

## Purpose

This document defines the proposed financial accounting architecture for DEYPAY.

It establishes the financial ledger, chart of accounts, accounting events, customer balance calculation, balance reservations, stablecoin deposit accounting, internal transfers, withdrawals, currency conversion, Nigerian bank payments, settlement, refunds, reversals, fees and reconciliation.

It also defines the financial invariants that must be enforced by the Rust backend and PostgreSQL database.

The intended audience includes software engineers, financial systems architects, accountants, treasury personnel, compliance officers, auditors, quality assurance engineers and technical partners.

This document is a technical accounting design. It does not replace a formal accounting policy approved by a qualified accountant, the applicable legal custody arrangement, or Nigerian regulatory advice.

## Document Authority

The FRS defines what DEYPAY must do.

The SAD defines how the application is organized.

The DDS defines how financial data is persisted.

This document defines how financial value is represented, moved, reserved, settled and reconciled.

No production financial posting implementation may be considered complete until its accounting behavior has been validated against this specification and the approved custody and settlement arrangements.

---

# TABLE OF CONTENTS

1. Executive Summary
2. Financial Operating Model
3. Ledger Objectives
4. Fundamental Accounting Principles
5. Ledger Ownership and Authority
6. Financial Asset and Unit Model
7. Chart of Accounts
8. Ledger Data Structures
9. Debit and Credit Rules
10. Customer Balance Model
11. Reservation Accounting
12. Customer Wallet Provisioning
13. Stablecoin Deposit Accounting
14. Internal Transfer Accounting
15. External Stablecoin Withdrawal Accounting
16. Stablecoin Conversion Accounting
17. NGN On-Ramp Accounting
18. NGN Off-Ramp Accounting
19. Pay Anyone Accounting
20. Prefunded NGN Settlement Accounting
21. Fee and Revenue Accounting
22. Refund and Reversal Accounting
23. Failed and Uncertain Transactions
24. Provider Settlement Accounting
25. Financial Transaction State Machines
26. Idempotency and Duplicate Prevention
27. Concurrency and Double-Spending Prevention
28. Journal Posting Engine
29. Database Integrity Enforcement
30. Financial Reconciliation
31. Treasury and Liquidity Management
32. Custody and Customer Asset Safeguarding
33. Administrative Financial Controls
34. Audit and Financial Reporting
35. Security Requirements
36. Failure Recovery
37. Worked Accounting Examples
38. Rust Implementation Design
39. Automated Financial Testing
40. Performance and Reliability
41. Implementation Sequence
42. Accounting and Operational Risks
43. Open Decisions and Dependencies
44. Acceptance Criteria
45. Final Ledger Architecture Definition

---

# 1. EXECUTIVE SUMMARY

DEYPAY will maintain its own independent double-entry financial ledger.

The ledger will serve as the authoritative internal accounting record for customer financial entitlements and platform financial positions.

External providers, including Nuvion where approved, may hold, transfer, convert or settle underlying financial assets.

However, provider balances and transaction histories must not replace DEYPAY's internal accounting records.

The financial system will distinguish between:

1. Customer liabilities.
2. Custodied or safeguarded assets.
3. Settlement assets.
4. Treasury assets.
5. Revenue.
6. Expenses.
7. Clearing balances.
8. Suspense balances.
9. Financial reservations.

USDC and USDT are the initial customer wallet assets.

NGN is used for fiat conversion, settlement and Nigerian bank payouts.

The MVP does not include a general-purpose customer NGN stored-value wallet.

All financial movements must be recorded using balanced accounting journals.

Every financial operation must be traceable to a business transaction, accounting event and supporting evidence.

Financial operations involving external providers must use durable transaction state machines, idempotency and reconciliation.

No customer balance may be changed through a direct, unaccounted database update.

---

# 2. FINANCIAL OPERATING MODEL

## 2.1 Custodial Wallet Model

DEYPAY's proposed MVP uses a custodial stablecoin wallet model.

Customers hold contractual entitlements to supported stablecoins through the approved custody arrangement.

The legal identity of the custodian, ownership of underlying assets, customer rights, segregation requirements and insolvency treatment must be established contractually and legally.

The accounting model assumes DEYPAY must track customer entitlements and the corresponding underlying assets or custodial claims.

The final presentation of assets and liabilities in statutory financial statements depends on the approved legal and accounting treatment.

## 2.2 Customer Assets

Initial supported customer assets:

- USDC.
- USDT.

Each customer may have separate accounting balances for each asset.

## 2.3 NGN Settlement

NGN may be used for:

- Customer funding.
- Stablecoin conversion.
- Bank withdrawals.
- Third-party bank payments.
- Provider settlement.
- Treasury liquidity.

NGN settlement balances must not automatically be presented as customer NGN wallet balances.

## 2.4 Internal Transfers

Internal DEYPAY transfers are ledger movements between customer liability accounts.

They do not require an on-chain transfer when both customer claims are held within the same approved custodial structure.

## 2.5 External Payments

External payments may require:

- Stablecoin reservation.
- Stablecoin conversion.
- NGN liquidity.
- Licensed bank payout infrastructure.
- Settlement confirmation.
- Reconciliation.

These operations cannot be treated as one atomic transaction across independent external systems.

---

# 3. LEDGER OBJECTIVES

The ledger must:

1. Accurately represent customer financial entitlements.
2. Maintain balanced accounting journals.
3. Prevent unauthorized balance changes.
4. Prevent double spending.
5. Support USDC and USDT independently.
6. Support NGN settlement accounting.
7. Record internal transfers atomically.
8. Support external deposits and withdrawals.
9. Track conversions.
10. Track third-party bank payments.
11. Record fees and revenue.
12. Track unsettled provider operations.
13. Support refunds and reversals.
14. Detect discrepancies.
15. Support financial reporting.
16. Remain independent of individual providers.
17. Support financial audits.
18. Recover safely from failures.

---

# 4. FUNDAMENTAL ACCOUNTING PRINCIPLES

## 4.1 Double-Entry Accounting

Every financial event must produce balanced journal entries.

For each asset:

Total debits = Total credits.

A journal containing multiple assets must balance independently within each asset.

## 4.2 Immutability

Posted journals and entries must not be edited or deleted.

Corrections require new compensating entries.

## 4.3 Exact Arithmetic

Financial calculations must use exact integer or decimal representations.

Floating-point arithmetic must never be used for financial postings.

## 4.4 Atomic Posting

All entries belonging to one journal must be committed atomically.

## 4.5 Traceability

Every posted journal must reference an identifiable accounting event.

## 4.6 Separation of Business and Accounting State

A business transaction status does not replace the accounting record.

A payment may be processing while funds are reserved.

A provider may report success while final reconciliation remains pending.

## 4.7 No Unexplained Value Creation

The system must not increase customer entitlements without a corresponding recognized asset, receivable, liability reduction or other valid accounting counter-entry.

## 4.8 Reconciliation

Ledger balances must be compared against independent financial evidence.

---

# 5. LEDGER OWNERSHIP AND AUTHORITY

## 5.1 DEYPAY-Owned Ledger

DEYPAY will own:

- Ledger account identifiers.
- Customer accounting relationships.
- Journal identifiers.
- Financial posting rules.
- Balance projections.
- Reservation logic.
- Accounting event references.
- Reconciliation records.

## 5.2 Provider Independence

Nuvion and other providers may supply:

- Custody infrastructure.
- Blockchain transactions.
- Currency conversion.
- Settlement services.
- External financial statements.

These providers do not define DEYPAY's internal chart of accounts.

## 5.3 Ledger Authority

The internal ledger is authoritative for DEYPAY's accounting records.

However, an internal ledger entry is not independent proof that the corresponding external assets exist.

Custody and settlement records must be reconciled to establish asset backing.

## 5.4 Legal Responsibility

DEYPAY's responsibility for customer assets must be consistent with its actual legal and contractual operating model.

If a regulated partner is the legal custodian or account provider, the ledger and customer disclosures must reflect that arrangement.

---

# 6. FINANCIAL ASSET AND UNIT MODEL

## 6.1 Asset Registry

Every accounting asset must have:

- Asset ID.
- Symbol.
- Name.
- Accounting precision.
- Asset type.
- Status.

## 6.2 Initial Assets

USDC.

USDT.

NGN.

## 6.3 Accounting Units

Recommended internal representation:

`NUMERIC(38,0)` integer accounting units.

Examples using six decimal places:

1 USDC = 1,000,000 accounting units.

25 USDC = 25,000,000 accounting units.

The actual precision must be configured per asset.

## 6.4 Network Token Precision

Blockchain token precision and internal ledger precision are separate concepts.

A token received on a supported blockchain must be normalized using its verified contract metadata and DEYPAY's approved accounting conversion rules.

## 6.5 Rounding

Rounding rules must be explicitly defined.

Rounding must never silently create or destroy customer value.

Any recognized rounding differences must be accounted for using approved accounts and policies.

## 6.6 Asset Isolation

USDC and USDT balances must never be combined without an explicit conversion.

NGN balances must not be treated as stablecoin balances.

---

# 7. CHART OF ACCOUNTS

## 7.1 Account Categories

The ledger will support:

- Assets.
- Liabilities.
- Revenue.
- Expenses.
- Equity.
- Clearing.
- Suspense.

Clearing and suspense are operational classifications and must map to the appropriate formal accounting categories.

## 7.2 Proposed Account Codes

The following is the initial logical chart of accounts.

### Asset Accounts

| Code | Account | Asset |
|---|---|---|
| 1100 | Custody USDC Assets | USDC |
| 1110 | Custody USDT Assets | USDT |
| 1120 | Stablecoin Provider Receivables | USDC/USDT |
| 1130 | NGN Settlement Bank Assets | NGN |
| 1140 | NGN Provider Settlement Receivables | NGN |
| 1150 | Treasury Stablecoin Assets | USDC/USDT |
| 1160 | Treasury NGN Assets | NGN |

### Liability Accounts

| Code | Account | Asset |
|---|---|---|
| 2100 | Customer Available USDC | USDC |
| 2110 | Customer Reserved USDC | USDC |
| 2120 | Customer Available USDT | USDT |
| 2130 | Customer Reserved USDT | USDT |
| 2140 | Pending Customer Deposit Obligations | USDC/USDT |
| 2150 | Pending Customer Refund Obligations | USDC/USDT |
| 2160 | NGN Payout Settlement Obligations | NGN |

### Revenue Accounts

| Code | Account | Asset |
|---|---|---|
| 4100 | Transfer Fee Revenue | USDC/USDT |
| 4110 | Withdrawal Fee Revenue | USDC/USDT |
| 4120 | Conversion Revenue | USDC/USDT/NGN |
| 4130 | Bank Payment Fee Revenue | USDC/USDT/NGN |

### Expense Accounts

| Code | Account | Asset |
|---|---|---|
| 5100 | Blockchain Network Expenses | USDC/USDT or actual fee asset |
| 5110 | Provider Processing Expenses | Applicable asset |
| 5120 | Conversion Expenses | Applicable asset |
| 5130 | Settlement Expenses | NGN |

### Clearing and Suspense Accounts

| Code | Account | Asset |
|---|---|---|
| 6100 | Stablecoin Conversion Clearing | USDC/USDT |
| 6110 | NGN Conversion Clearing | NGN |
| 6120 | Bank Payout Clearing | NGN |
| 6130 | External Withdrawal Clearing | USDC/USDT |
| 6140 | Unidentified Deposit Suspense | USDC/USDT |
| 6150 | Reconciliation Suspense | Applicable asset |

## 7.3 Account Instances

Account codes represent logical account categories.

Individual customer accounts must have unique internal ledger account identifiers.

For example:

Customer A — USDC Available.

Customer A — USDC Reserved.

Customer B — USDC Available.

Customer B — USDC Reserved.

Each must be a distinct ledger account.

## 7.4 Accounting Classification

The proposed chart is an operational subledger design.

The final statutory general ledger mappings, valuation policies, revenue recognition rules and financial statement classifications must be approved by qualified accounting personnel.

---

# 8. LEDGER DATA STRUCTURES

## 8.1 Ledger Account

Each account must contain:

- Account ID.
- Account code.
- Owner.
- Asset.
- Account type.
- Normal balance.
- Status.
- Creation timestamp.

## 8.2 Journal

Each journal must contain:

- Journal ID.
- Journal reference.
- Accounting event.
- Business transaction reference.
- Posting status.
- Posting timestamp.
- Reversal reference.

## 8.3 Journal Entry

Each entry must contain:

- Entry ID.
- Journal ID.
- Ledger account ID.
- Asset.
- Direction.
- Amount.
- Timestamp.

## 8.4 Balance Projection

Balance projections are derived representations of posted entries.

They must be reconstructable from the journal history.

## 8.5 Reservation

A reservation identifies funds committed to an operation but not yet finally settled.

## 8.6 Accounting Event

An accounting event identifies the business event that caused a financial posting.

Examples:

- DEPOSIT_CONFIRMED.
- INTERNAL_TRANSFER_EXECUTED.
- WITHDRAWAL_RESERVED.
- WITHDRAWAL_SETTLED.
- BANK_PAYMENT_RESERVED.
- CONVERSION_SETTLED.
- REFUND_POSTED.

---

# 9. DEBIT AND CREDIT RULES

## 9.1 Asset Accounts

Debit increases an asset.

Credit decreases an asset.

## 9.2 Liability Accounts

Credit increases a liability.

Debit decreases a liability.

## 9.3 Revenue Accounts

Credit increases revenue.

Debit decreases revenue.

## 9.4 Expense Accounts

Debit increases expense.

Credit decreases expense.

## 9.5 Equity Accounts

Credit generally increases equity.

Debit generally decreases equity.

## 9.6 Balance Calculation

The account's normal balance determines how debits and credits affect its displayed balance.

The ledger engine must not assume that every debit means money leaves a customer.

For customer liability accounts, debits generally reduce the amount owed to the customer.

---

# 10. CUSTOMER BALANCE MODEL

## 10.1 Balance Categories

Each customer wallet should expose:

- Available balance.
- Reserved balance.
- Total recognized customer entitlement.

## 10.2 Available Balance

Available balance represents funds eligible for spending.

## 10.3 Reserved Balance

Reserved balance represents funds committed to pending operations.

## 10.4 Total Balance

For the selected reserved-account model:

Total customer entitlement = Available liability balance + Reserved liability balance.

## 10.5 Selected Reservation Architecture

DEYPAY will use dedicated reserved ledger liability accounts.

This model applies to all customer-funded external executions.

The separate reservation record tracks workflow ownership and lifecycle.

The reserved ledger account represents the financial reservation.

## 10.6 Avoiding Double Counting

Because reserved amounts are already moved between ledger accounts, the application must not subtract reservation records again when calculating available balances.

---

# 11. RESERVATION ACCOUNTING

## 11.1 Purpose

Reservations prevent funds from being spent while an external financial operation is pending.

## 11.2 Example

A customer has 100 USDT available.

The customer initiates a 30 USDT withdrawal.

Reservation journal:

Debit: Customer Available USDT — 30.

Credit: Customer Reserved USDT — 30.

After posting:

Available: 70 USDT.

Reserved: 30 USDT.

Total entitlement: 100 USDT.

## 11.3 Reservation Settlement

When the external withdrawal is confirmed, the reserved liability is extinguished against the appropriate asset or clearing account.

## 11.4 Reservation Release

If the operation is definitively cancelled before external execution:

Debit: Customer Reserved USDT — 30.

Credit: Customer Available USDT — 30.

## 11.5 Unknown Outcome

While execution of the reserved leg is uncertain, retain the reservation. After confirmed conversion, protect the resulting settlement obligation under §19.

The system must not automatically release funds solely because of an API timeout.

## 11.6 Reservation Identity

Each reservation must have a unique relationship to the financial operation.

Repeated reservation requests must not create multiple reservations for the same operation.

---

# 12. CUSTOMER WALLET PROVISIONING

## 12.1 Wallet Creation

When an eligible customer receives a USDC or USDT wallet profile, the system creates the required internal ledger accounts.

## 12.2 Initial Balance

New wallet accounts begin with zero balances.

## 12.3 Accounting Impact

Wallet profile creation does not create financial value.

No monetary journal is required merely because a wallet profile is created.

## 12.4 Provider Provisioning

External address provisioning is a separate technical operation.

A blockchain address must not be treated as evidence of a funded wallet.

---

# 13. STABLECOIN DEPOSIT ACCOUNTING

## 13.1 Deposit Lifecycle

A deposit passes through:

1. Detection.
2. Verification.
3. Confirmation.
4. Compliance screening.
5. Accounting recognition.
6. Customer availability.

## 13.2 Deposit Recognition

A deposit must not become spendable until the configured recognition criteria are satisfied.

## 13.3 Example

A customer deposits 50 USDC.

Once the deposit is verified and recognized:

Debit: Custody USDC Assets — 50.

Credit: Customer Available USDC — 50.

## 13.4 Accounting Result

Custody asset increases by 50 USDC.

Customer liability increases by 50 USDC.

The journal balances.

## 13.5 Pending Deposits

A detected but unconfirmed deposit may be recorded in operational deposit tables without creating a spendable customer liability.

If accounting policy requires recognition before availability, separate pending accounts must be used.

## 13.6 Duplicate Prevention

The same blockchain transfer must never be credited twice.

A pending deposit is informational and excluded from spendable funds until confirmation, screening and customer eligibility checks pass.

The posting engine must enforce uniqueness using the approved deposit identity.

## 13.7 Deposit Reorganization

If a blockchain reorganization invalidates a previously recognized deposit, the system must execute the approved exception and recovery process.

Posted journals must not be deleted.

---

# 14. INTERNAL TRANSFER ACCOUNTING

## 14.1 Purpose

Internal transfers move customer entitlements between DEYPAY users.

## 14.2 Example

Customer A sends 25 USDC to Customer B.

Journal:

Debit: Customer A Available USDC — 25.

Credit: Customer B Available USDC — 25.

## 14.3 Result

Customer A's entitlement decreases by 25 USDC.

Customer B's entitlement increases by 25 USDC.

The aggregate customer liability remains unchanged.

Custody assets remain unchanged.

## 14.4 Atomicity

The debit and credit must occur in the same PostgreSQL transaction.

## 14.5 Concurrency

The sender's available balance must be locked or otherwise protected against concurrent overspending.

## 14.6 Fees

If DEYPAY introduces an internal transfer fee, it must be recorded separately.

A zero-fee internal transfer produces no fee revenue.

---

# 15. EXTERNAL STABLECOIN WITHDRAWAL ACCOUNTING

## 15.1 Withdrawal Lifecycle

1. Customer requests withdrawal.
2. System validates eligibility.
3. System validates balance.
4. Customer authorizes.
5. Funds are reserved.
6. Provider executes.
7. Blockchain settlement is verified.
8. Accounting is finalized.

## 15.2 Example

Customer withdraws 20 USDT.

Reservation:

Debit: Customer Available USDT — 20.

Credit: Customer Reserved USDT — 20.

## 15.3 Settlement

When the external transfer is confirmed:

Debit: Customer Reserved USDT — 20.

Credit: Custody USDT Assets — 20.

## 15.4 Result

Customer liability decreases by 20 USDT.

Custody assets decrease by 20 USDT.

## 15.5 Withdrawal Fees

If a customer pays a separate withdrawal fee, that fee must be reserved and recognized according to the approved fee policy.

For the API example of 50 USDT plus a 2 USDT earned platform fee, reserve 52 (debit available, credit reserved). On verified settlement debit reserved 52, credit custody 50 and fee revenue 2. On verified non-execution with no fee earned, debit reserved 52 and credit available 52. Actual provider costs follow §21.5.

## 15.6 Network Fees

Blockchain network fees must be accounted for using the actual asset consumed.

A native network token fee must not be incorrectly recorded as a USDT expense.

## 15.7 Failed Withdrawal

A confirmed failure before external settlement may release the reservation.

An uncertain outcome requires reconciliation.

---

# 16. STABLECOIN CONVERSION ACCOUNTING

## 16.1 Purpose

Conversion allows customers to exchange one supported asset for another.

Example:

USDC to USDT.

## 16.2 Accounting Requirement

Each asset must balance independently.

A USDC debit cannot be directly balanced by a USDT credit.

## 16.3 Example

A customer converts 100 USDC into 99.5 USDT.

Assume:

- The conversion provider accepts 100 USDC.
- The provider delivers 99.5 USDT.
- All applicable fees are included in the quote.

Reserve 100 USDC before submission: debit Customer Available USDC and credit Customer Reserved USDC.

USDC settlement journal:

Debit: Customer Reserved USDC — 100.

Credit: Custody USDC Assets — 100.

USDT journal:

Debit: Custody USDT Assets — 99.5.

Credit: Customer Available USDT — 99.5.

## 16.4 Settlement Timing

If conversion execution and delivery are not simultaneous, provider receivable and clearing accounts must be used.

## 16.5 Fee Recognition

Explicit DEYPAY fees must be recorded separately.

Provider costs and spread revenue must follow the approved accounting policy.

## 16.6 Quote Validation

The executed conversion must be traceable to an accepted and valid quote.

---

# 17. NGN ON-RAMP ACCOUNTING

## 17.1 Purpose

NGN on-ramp allows customers to fund stablecoin balances through an approved fiat-to-stablecoin arrangement.

## 17.2 No General NGN Wallet

The customer does not receive a general-purpose NGN stored-value balance in the initial model.

## 17.3 Example

A customer pays ₦150,000 and receives 100 USDT.

Assume the approved provider receives the NGN and delivers 100 USDT into the custody arrangement.

NGN settlement accounting:

Debit: NGN Settlement Bank Assets or Provider Receivable — ₦150,000.

Credit: NGN Conversion Clearing — ₦150,000.

USDT accounting:

Debit: Custody USDT Assets — 100.

Credit: Customer Available USDT — 100.

The NGN conversion clearing balance must subsequently be settled through the corresponding NGN payment, provider payable or other approved conversion entries.

## 17.4 Recognition

Customer USDT must not become available until the required payment and stablecoin delivery conditions are satisfied.

## 17.5 Provider Dependency

The precise on-ramp accounting depends on whether DEYPAY, a regulated partner or a settlement intermediary receives the NGN and delivers the stablecoin.

---

# 18. NGN OFF-RAMP ACCOUNTING

## 18.1 Purpose

NGN off-ramp enables customers to convert stablecoins into NGN paid to their own verified bank account.

## 18.2 Processing

1. Customer requests off-ramp.
2. Bank ownership and eligibility are checked.
3. Quote is accepted.
4. Stablecoins are reserved.
5. Conversion occurs.
6. NGN payout is initiated.
7. Payout is confirmed.
8. Accounting is finalized.

## 18.3 Example

Customer uses 20 USDT to receive ₦30,000.

Assume no separately charged fee.

The stablecoin liability must be reserved and extinguished according to the confirmed conversion execution.

The NGN settlement obligation must be created when the provider becomes obligated to deliver the payout.

The NGN settlement asset or provider receivable must be reduced when payout settlement is confirmed.

## 18.4 Important Distinction

Conversion completion and bank payout completion are separate events.

The system must not assume that successful conversion means the customer has received NGN.

---

# 19. PAY ANYONE ACCOUNTING

## 19.1 Purpose

Pay Anyone allows a customer to pay a third-party Nigerian bank account using USDC or USDT.

## 19.2 Example

A customer pays a grocery store ₦30,000.

The customer funds the payment using 20 USDT.

Assume:

- Conversion rate: ₦1,500 per USDT.
- No additional customer fee.
- The provider confirms delivery of ₦30,000.
- The values are illustrative, not live market rates.

## 19.3 Reservation

Debit: Customer Available USDT — 20.

Credit: Customer Reserved USDT — 20.

## 19.4 Conversion Settlement

Upon confirmed conversion:

Debit: Customer Reserved USDT — 20.

Credit: Custody USDT Assets — 20.

NGN asset recognition:

Debit: NGN Settlement Bank Assets or Provider Receivable — ₦30,000.

Credit: NGN Payout Settlement Obligations — ₦30,000.

## 19.5 Bank Payout Settlement

When the payout is confirmed:

Debit: NGN Payout Settlement Obligations — ₦30,000.

Credit: NGN Settlement Bank Assets or Provider Receivable — ₦30,000, according to the asset actually used for payout.

## 19.6 Result

The customer's USDT entitlement decreases by 20.

The grocery store receives ₦30,000.

The NGN settlement obligation is extinguished.

## 19.7 Important Accounting Qualification

The example assumes that the conversion provider delivers the NGN asset or an enforceable settlement receivable before the payout obligation is settled.

If a prefunded settlement pool is used, or the conversion and payout are executed by different entities, additional clearing, treasury and provider settlement entries are required.

The actual journal mapping must reflect the legal counterparty and settlement flow.

## 19.8 Failure Handling

A bank payout failure after conversion must not automatically recreate the original USDT balance.

The system must determine whether the customer is owed:

- A stablecoin refund.
- A NGN refund or settlement claim.
- A replacement payout.
- Another contractually approved remedy.

The refund accounting must follow the actual assets held and obligations outstanding.

A timeout after confirmed conversion leaves the NGN obligation outstanding, with no release or refund journal. On definitive payout failure, retain it or reclassify it by debiting NGN payout obligations and crediting NGN refund payable for the same amount. Discharge it once on verified replacement payout or refund. A stablecoin refund requires evidenced funding or reconversion and separately balanced journals in each asset. Finance and Product must approve refund denomination, fee treatment and allocation of FX gains or losses for the contracted route before live activation.

---

# 20. PREFUNDED NGN SETTLEMENT ACCOUNTING

## 20.1 Purpose

DEYPAY may maintain prefunded NGN settlement liquidity through an approved provider.

## 20.2 Treasury Funding

When NGN treasury funds are transferred to a payout provider, the ledger must distinguish between:

- NGN held in DEYPAY-controlled bank accounts.
- NGN held by the payout provider.
- NGN committed to customer payments.
- NGN already paid out.

## 20.3 Example

DEYPAY transfers ₦5,000,000 to an approved payout provider.

Debit: NGN Provider Settlement Receivable — ₦5,000,000.

Credit: Treasury NGN Assets — ₦5,000,000.

This assumes the provider holds the funds on DEYPAY's behalf and the resulting receivable is appropriately recognized.

## 20.4 Payout Funding

As bank payouts settle, the relevant provider settlement asset or receivable decreases.

## 20.5 Liquidity Monitoring

The system must track:

- Total provider settlement balance.
- Available settlement liquidity.
- Pending payout commitments.
- Unsettled conversion proceeds.
- Reconciliation differences.

## 20.6 Customer Asset Protection

Customer custodial assets and DEYPAY treasury assets must be distinguished.

Customer funds must not be used as unrestricted corporate operating capital.

---

# 21. FEE AND REVENUE ACCOUNTING

## 21.1 Fee Categories

Potential fee categories:

- External withdrawal fees.
- Bank payment fees.
- Conversion fees.
- Conversion spread.
- Provider service fees.

## 21.2 Fee Transparency

Customer-facing quotes must disclose applicable charges according to the approved pricing policy.

## 21.3 Fee Recognition

Fees must be recognized only when the relevant performance obligation and accounting recognition conditions are satisfied.

## 21.4 Example

A customer pays a 1 USDT withdrawal fee.

Debit: Customer Reserved USDT — 1 (fee reserved with principal under §15.5).

Credit: Withdrawal Fee Revenue — 1.

This journal reduces the customer liability and recognizes revenue.

## 21.5 Provider Expenses

Provider costs must be recorded separately from DEYPAY revenue unless the approved accounting treatment requires net presentation.

## 21.6 Revenue Reconciliation

Fee revenue must be reconcilable to individual financial transactions.

---

# 22. REFUND AND REVERSAL ACCOUNTING

## 22.1 Refund

A refund returns value to the customer according to an approved financial outcome.

## 22.2 Reversal

A reversal corrects or offsets a previously posted accounting event.

## 22.3 Immutability

Original posted entries must remain intact.

## 22.4 Compensating Journal

A reversal must create a new journal referencing the original journal.

## 22.5 Example

A 10 USDC internal transfer must be reversed after an approved investigation.

Original:

Debit: Sender Available USDC — 10.

Credit: Recipient Available USDC — 10.

Reversal:

Debit: Recipient Available USDC — 10.

Credit: Sender Available USDC — 10.

## 22.6 Insufficient Recipient Funds

The system must not assume a reversal is possible if the recipient has already spent the funds.

Such cases require an approved recovery, receivable, restriction or loss recognition process.

## 22.7 Bank Payment Refunds

Refunds after NGN conversion must reflect the actual settlement assets and contractual obligations.

---

# 23. FAILED AND UNCERTAIN TRANSACTIONS

## 23.1 Failure Categories

- Validation failure.
- Authorization failure.
- Insufficient balance.
- Provider rejection.
- Provider timeout.
- Blockchain failure.
- Bank payout rejection.
- Reconciliation mismatch.
- Settlement uncertainty.

## 23.2 Definitive Failure

A definitive failure may permit the release of reserved funds.

## 23.3 Unknown Outcome

An unknown outcome must preserve sufficient funds and operation state until reconciliation determines the result.

## 23.4 Retry Rules

Retries must not create duplicate external payments.

## 23.5 Financial Recovery

Recovery actions must be driven by verified financial evidence.

---

# 24. PROVIDER SETTLEMENT ACCOUNTING

## 24.1 Provider Operations

Every external financial operation must have an internal provider operation record.

## 24.2 External References

Provider references must be retained.

## 24.3 Settlement Status

The system must distinguish:

- Submitted.
- Accepted.
- Processing.
- Executed.
- Settled.
- Failed.
- Reversed.
- Unknown.

## 24.4 Reconciliation

Provider balances and transaction statements must be matched to internal accounting records.

## 24.5 Provider Independence

Nuvion's accounting records may provide external evidence but must not replace DEYPAY's internal journal system.

---

# 25. FINANCIAL TRANSACTION STATE MACHINES

## 25.1 Internal Transfer

CREATED → VALIDATED → AUTHORIZED → COMPLETED.

Failure states:

REJECTED.

FAILED.

## 25.2 Deposit

DETECTED → CONFIRMING → SCREENING → CREDITED.

Exceptions: UNDER_REVIEW, REJECTED, FAILED. A verified invalidation of a recognized deposit enters REORG_REVIEW. Use REVERSED only after an approved compensating journal.

## 25.3 Withdrawal

CREATED → QUOTED → AUTHORIZED → RESERVED → SUBMITTED → PROCESSING → COMPLETED. Track broadcast and chain confirmations as execution evidence within PROCESSING. An uncertain submission enters PENDING_RECONCILIATION; a verified pre-settlement failure can enter FAILED. Use CANCELLED or EXPIRED only where execution is ruled out.

## 25.4 Bank Payment

CREATED → BENEFICIARY_VERIFIED → QUOTED → AUTHORIZED → RESERVED → CONVERTING → PAYOUT_PROCESSING → COMPLETED. Applies to own-bank and Pay Anyone payouts. Confirmed conversion consumes the stablecoin reserve and creates the NGN obligation (§19). After conversion, confirmed payout failure enters REFUND_PROCESSING or UNDER_REVIEW pending an approved remedy. Unknown execution enters PENDING_RECONCILIATION. Use REFUNDED only with a verified refund and compensating postings. An approved replacement can return to PAYOUT_PROCESSING only after definitive non-execution of the earlier payout. It uses the outstanding NGN obligation and a separately identified payout attempt.

## 25.5 State Transition Rules

For standalone FX use CREATED → QUOTED → AUTHORIZED → RESERVED → CONVERTING → COMPLETED. For NGN funding use CREATED → QUOTED → AUTHORIZED → AWAITING_FUNDS → CONVERTING → CREDITED. These paths describe accepted operations; pre-acceptance rejection, cancellation or expiry requires evidence of non-execution. Record the prior state, evidence and transition atomically with any postings.

| Operation | Evidence required for CREDITED or COMPLETED |
|---|---|
| Internal transfer | Committed balanced sender and recipient posting. |
| Deposit or NGN funding | Verified eligible chain transfer with finality and screening, or confirmed NGN collection and stablecoin delivery; one customer credit. |
| External withdrawal | Verified configured network settlement and posted consumption of the reserve. |
| Standalone FX | Verified settlement of both asset legs and their balanced postings. |
| Bank payment | Verified beneficiary payout and settlement journal discharging the NGN obligation. |

Use FAILED only for definitive failure with the financial effect accounted for. UNDER_REVIEW holds the affected step. PENDING_RECONCILIATION can resume the evidenced step or close on verified settlement or non-execution. Keep unsettled obligations open. Late returns, reorgs and refunds follow an evidenced recovery path and preserve prior postings. Public mappings are in 04-API-SPECIFICATION.md §38.

---

# 26. IDEMPOTENCY AND DUPLICATE PREVENTION

## 26.1 Financial Idempotency

Every financial operation must have a unique internal identity.

## 26.2 Request Idempotency

Repeated requests using the same idempotency key must not execute the operation twice.

## 26.3 Accounting Idempotency

Each accounting event must have a unique posting reference.

Use distinct identities for the customer request, internal operation, provider submission, verified economic event and resulting posting leg (03-DATABASE-DESIGN.md §§13.5, 16.2, 22.3, 31). Repeated observations of one fact reuse the posting key, including after a restart.

## 26.4 Provider Idempotency

Provider idempotency keys must be used where supported.

## 26.5 Webhook Idempotency

Repeated provider events must not cause duplicate postings.

## 26.6 Reconciliation

Duplicate external execution must be detected even when the provider does not support idempotency.

---

# 27. CONCURRENCY AND DOUBLE-SPENDING PREVENTION

## 27.1 Problem

A customer with 100 USDT may submit two simultaneous 80 USDT payments.

Without proper controls, both could incorrectly succeed.

## 27.2 Required Behavior

Only one payment may reserve the available funds.

The other must fail or wait and re-evaluate the balance.

## 27.3 PostgreSQL Controls

Use:

- Database transactions.
- Row-level locking.
- Consistent lock ordering.
- Unique constraints.
- Appropriate isolation levels.

## 27.4 Atomic Reservations

Balance validation and reservation posting must occur in one protected transaction.

## 27.5 Deadlock Handling

Deadlocks must be detected and safely retried when appropriate.

---

# 28. JOURNAL POSTING ENGINE

## 28.1 Purpose

The posting engine is the only approved application interface for creating posted financial journals.

## 28.2 Inputs

- Accounting event ID.
- Journal reference.
- Entries.
- Asset.
- Amounts.
- Source transaction.
- Metadata.

## 28.3 Validation

The engine must validate:

1. Unique accounting event.
2. Valid accounts.
3. Valid assets.
4. Positive amounts.
5. Balanced entries.
6. Account status.
7. Available balance requirements.
8. Posting permissions.
9. Valid reversal references.

## 28.4 Posting

The engine must:

1. Begin a database transaction.
2. Lock affected accounts.
3. Validate balances.
4. Insert journal.
5. Insert entries.
6. Update projections.
7. Record accounting event.
8. Insert outbox event.
9. Commit.

## 28.5 Failure

If any step fails, the database transaction must roll back.

No partial journal may be committed.

---

# 29. DATABASE INTEGRITY ENFORCEMENT

## 29.1 Database Constraints

Use:

- Primary keys.
- Foreign keys.
- Unique constraints.
- Check constraints.
- Controlled privileges.

## 29.2 Journal Balance Enforcement

The database design must ensure that unbalanced journals cannot become posted.

## 29.3 Immutable Entries

Posted journal entries must not be updateable or deletable by ordinary application roles.

## 29.4 Posting Interface

A controlled database function or carefully restricted posting service may be used.

The final mechanism must be selected during implementation.

## 29.5 Financial Projections

Balance projections must be updated atomically with journal posting.

---

# 30. FINANCIAL RECONCILIATION

## 30.1 Reconciliation Objectives

Reconciliation must confirm:

- Customer liabilities.
- Custody assets.
- Provider balances.
- Pending transfers.
- Settlement obligations.
- Revenue.
- Expenses.

## 30.2 Reconciliation Levels

### Transaction-Level Reconciliation

Match individual transactions.

### Balance-Level Reconciliation

Compare aggregate balances.

### Settlement Reconciliation

Verify external payout and conversion settlement.

### Accounting Reconciliation

Verify internal journal and account balances.

## 30.3 Customer Liability Reconciliation

Aggregate customer liabilities must be compared with corresponding custody assets and permitted settlement positions.

Any shortfall must be investigated immediately.

## 30.4 Discrepancy Cases

Discrepancies must have:

- Unique ID.
- Severity.
- Financial impact.
- Assigned owner.
- Investigation status.
- Resolution evidence.

## 30.5 Reconciliation Frequency

Recommended initial policy:

- Continuous event-level checks.
- Frequent operational reconciliation.
- Daily formal reconciliation.
- Periodic independent financial review.

Exact frequency must be established based on transaction volume, provider capabilities and regulatory requirements.

---

# 31. TREASURY AND LIQUIDITY MANAGEMENT

## 31.1 Treasury Purpose

Treasury ensures sufficient liquidity exists to fulfill approved financial obligations.

## 31.2 Liquidity Categories

- USDC custody liquidity.
- USDT custody liquidity.
- NGN settlement liquidity.
- Provider prefunding.
- Pending conversion receivables.
- Pending payout obligations.

## 31.3 Treasury Monitoring

Monitor:

- Available balances.
- Committed balances.
- Pending settlements.
- Provider exposure.
- Concentration risk.
- Liquidity thresholds.

## 31.4 Liquidity Alerts

The system must alert operators before liquidity shortages cause widespread payment failures.

## 31.5 Treasury Separation

Treasury movements must be separately authorized and auditable.

---

# 32. CUSTODY AND CUSTOMER ASSET SAFEGUARDING

## 32.1 Custody Responsibilities

The legal custodian must be identified.

## 32.2 Customer Entitlements

Customer entitlements must be represented accurately.

## 32.3 Segregation

Customer assets must be distinguished from corporate treasury assets.

## 32.4 Insolvency Treatment

The legal treatment of customer assets during provider or company insolvency must be documented.

## 32.5 Custody Reconciliation

Custody balances must be independently reconciled against customer liabilities.

## 32.6 Provider Failure

The system must maintain enough internal records to support customer entitlement reconstruction if a provider becomes unavailable.

---

# 33. ADMINISTRATIVE FINANCIAL CONTROLS

## 33.1 No Direct Balance Editing

Administrators must not directly modify customer balances.

## 33.2 Adjustments

Adjustments must use approved accounting events.

## 33.3 Maker-Checker

Sensitive financial operations must require separate approval.

## 33.4 Audit Trail

Every adjustment must record:

- Requester.
- Approver.
- Reason.
- Supporting evidence.
- Journal reference.
- Timestamp.

## 33.5 Privilege Separation

Support agents must not automatically possess treasury or financial adjustment permissions.

---

# 34. AUDIT AND FINANCIAL REPORTING

## 34.1 Audit Records

The system must preserve:

- Accounting journals.
- Ledger entries.
- Transaction references.
- Provider evidence.
- Reconciliation results.
- Administrative approvals.

## 34.2 Financial Reports

Required reports include:

- Customer liability balances.
- Custody asset balances.
- Daily transaction totals.
- Fees and revenue.
- Outstanding withdrawals.
- Outstanding bank payments.
- Unsettled conversions.
- Reconciliation discrepancies.
- Treasury liquidity.

## 34.3 Report Reproducibility

Reports must be reproducible from retained accounting records.

---

# 35. SECURITY REQUIREMENTS

## 35.1 Ledger Access

Only authorized application services may initiate financial postings.

## 35.2 Database Privileges

Ordinary application roles must not be able to bypass posting controls.

## 35.3 Financial Authorization

High-risk financial operations must require appropriate authorization.

## 35.4 Logging

Financial logs must include correlation identifiers without exposing secrets.

## 35.5 Monitoring

Suspicious posting patterns must generate alerts.

---

# 36. FAILURE RECOVERY

## 36.1 Application Crash

Uncommitted database transactions must roll back.

## 36.2 Worker Crash

Durable jobs must resume safely.

## 36.3 Provider Timeout

The operation must remain in an uncertain state until verified.

## 36.4 Database Recovery

After restoring a database backup, external provider operations must be reconciled.

Resume affected execution only after matching evidence through cutover, rebuilding missing journals once and reconciling custody assets, customer liabilities and outstanding settlement obligations.

## 36.5 Duplicate Callback

Duplicate callbacks must not create duplicate journals.

## 36.6 Reversal Failure

Failed compensation must remain visible as an unresolved financial obligation.

---

# 37. WORKED ACCOUNTING EXAMPLES

The following examples illustrate the selected double-entry model.

They are simplified and exclude fees unless stated.

## Example A: Deposit 100 USDC

Debit: Custody USDC Assets — 100.

Credit: Customer A Available USDC — 100.

Result:

Customer A has 100 USDC available.

Custody assets increase by 100 USDC.

## Example B: Internal Transfer of 40 USDC

Customer A sends 40 USDC to Customer B.

Debit: Customer A Available USDC — 40.

Credit: Customer B Available USDC — 40.

Result:

Customer A has 60 USDC.

Customer B has 40 USDC.

Total customer liabilities remain 100 USDC.

## Example C: Reserve 20 USDC

Customer A initiates an external withdrawal.

Debit: Customer A Available USDC — 20.

Credit: Customer A Reserved USDC — 20.

Result:

Available: 40 USDC.

Reserved: 20 USDC.

Total entitlement: 60 USDC.

## Example D: Complete Withdrawal

Debit: Customer A Reserved USDC — 20.

Credit: Custody USDC Assets — 20.

Result:

Customer A has 40 USDC.

Custody assets decrease by 20 USDC.

## Example E: Cancel Reserved Withdrawal

If the withdrawal had not executed:

Debit: Customer A Reserved USDC — 20.

Credit: Customer A Available USDC — 20.

Result:

Available balance returns to 60 USDC.

Reserved balance becomes zero.

## Example F: Stablecoin Conversion

Customer converts 50 USDC into 49.8 USDT.

Reserve 50 USDC: debit Customer Available USDC and credit Customer Reserved USDC.

USDC settlement journal:

Debit: Customer Reserved USDC — 50.

Credit: Custody USDC Assets — 50.

USDT journal:

Debit: Custody USDT Assets — 49.8.

Credit: Customer Available USDT — 49.8.

Result:

USDC entitlement decreases by 50.

USDT entitlement increases by 49.8.

The actual provider settlement flow may require intermediate clearing accounts.

## Example G: Pay Nigerian Bank Account

Customer pays ₦15,000 using 10 USDT.

Reservation:

Debit: Customer Available USDT — 10.

Credit: Customer Reserved USDT — 10.

Conversion settlement:

Debit: Customer Reserved USDT — 10.

Credit: Custody USDT Assets — 10.

NGN settlement recognition:

Debit: NGN Settlement Assets — ₦15,000.

Credit: NGN Payout Settlement Obligations — ₦15,000.

Bank payout:

Debit: NGN Payout Settlement Obligations — ₦15,000.

Credit: NGN Settlement Assets — ₦15,000.

Result:

Customer USDT entitlement decreases by 10.

Recipient receives ₦15,000.

Settlement obligation is extinguished.

## Example H: Fee Recognition

Customer pays 0.5 USDT fee.

Debit: Customer Reserved USDT — 0.5 (fee previously reserved with the external payment).

Credit: Fee Revenue USDT — 0.5.

Result:

Customer liability decreases by 0.5 USDT.

DEYPAY recognizes fee revenue according to the approved accounting policy.

---

# 38. RUST IMPLEMENTATION DESIGN

## 38.1 Ledger Module

Recommended location:

`backend/crates/deypay-domain/src/ledger/`

## 38.2 Domain Types

Recommended types:

- LedgerAccountId.
- JournalId.
- LedgerEntryId.
- AccountingEventId.
- AssetId.
- Money.
- PostingDirection.
- LedgerAccountType.

## 38.3 Money Type

A Money value must contain:

- Asset identifier.
- Exact integer amount.

## 38.4 Posting Service

The application layer should expose a controlled posting service.

Conceptual interface:

```rust
pub struct PostingRequest {
    pub accounting_event_id: Uuid,
    pub source_transaction_id: Option<Uuid>,
    pub entries: Vec<PostingEntry>,
}

pub struct PostingEntry {
    pub account_id: Uuid,
    pub asset_id: Uuid,
    pub direction: EntryDirection,
    pub amount_minor: i128,
}
```

This is a conceptual example.

The final implementation must validate supported ranges and use SQLx-compatible exact numeric conversion.

## 38.5 Posting Result

A successful posting must return:

- Journal ID.
- Accounting event ID.
- Posting timestamp.
- Posting status.

## 38.6 Error Types

Recommended errors:

- InvalidAmount.
- AssetMismatch.
- UnbalancedJournal.
- InsufficientFunds.
- AccountRestricted.
- DuplicateAccountingEvent.
- InvalidReversal.
- ConcurrentModification.
- DatabaseFailure.

## 38.7 Database Transaction Boundary

The posting service must own the financial database transaction boundary or participate in an explicitly controlled application transaction.

No partial posting may be committed.

---

# 39. AUTOMATED FINANCIAL TESTING

## 39.1 Ledger Balance Tests

Every posted journal must balance by asset.

## 39.2 Immutability Tests

Posted entries cannot be edited or deleted.

## 39.3 Insufficient Balance Tests

A customer cannot spend more than the available balance.

## 39.4 Concurrent Transfer Tests

Multiple simultaneous transfers cannot overspend the same account.

## 39.5 Duplicate Event Tests

Repeated deposit events cannot credit the customer twice.

## 39.6 Reservation Tests

Funds cannot be reserved twice for the same operation.

## 39.7 Withdrawal Failure Tests

A confirmed failed withdrawal releases funds correctly.

## 39.8 Unknown Outcome Tests

A provider timeout does not automatically release funds.

## 39.9 Reversal Tests

Reversals preserve original journals.

## 39.10 Reconciliation Tests

Missing, duplicate and mismatched provider records are detected.

## 39.11 Property-Based Testing

Property-based testing should validate accounting invariants across randomized transaction sequences.

---

# 40. PERFORMANCE AND RELIABILITY

## 40.1 Correctness Priority

Financial correctness must take precedence over throughput.

## 40.2 Transaction Performance

Ledger posting must be efficient but must never bypass required locks or constraints.

## 40.3 Scalability

The ledger should support:

- Indexed journal lookups.
- Efficient balance projections.
- Batch reconciliation.
- Partitioning when justified.

## 40.4 Reliability Targets

Target:

- Zero unexplained ledger imbalance.
- Zero duplicate financial posting.
- Recoverable incomplete operations.
- Traceable external settlement.

These are mandatory correctness objectives rather than claims that failures are impossible.

---

# 41. IMPLEMENTATION SEQUENCE

## Phase 1: Accounting Foundations

- Finalize asset precision.
- Finalize chart of accounts.
- Finalize customer liability model.
- Validate the dedicated available and reserved account mappings.

## Phase 2: Ledger Database

- Ledger accounts.
- Journals.
- Entries.
- Balance projections.
- Accounting events.
- Reservations.

## Phase 3: Posting Engine

- Balanced posting.
- Account locking.
- Balance validation.
- Idempotency.
- Reversal handling.

## Phase 4: Internal Transfers

- Atomic transfers.
- Concurrency tests.
- Transaction history integration.

## Phase 5: Deposit Accounting

- Custody asset recognition.
- Customer liability recognition.
- Duplicate prevention.

## Phase 6: Withdrawal Accounting

- Reservations.
- External execution.
- Settlement.
- Failure recovery.

## Phase 7: Conversion Accounting

- Quotes.
- Clearing.
- Settlement.
- Fee recognition.

## Phase 8: Nigerian Payments

- NGN settlement assets.
- Payout obligations.
- Prefunding.
- Pay Anyone accounting.
- Reconciliation.

## Phase 9: Operational Controls

- Administrative adjustments.
- Maker-checker.
- Financial reporting.
- Audit records.

## Phase 10: Validation

- Automated financial tests.
- Concurrency tests.
- Failure tests.
- Provider reconciliation tests.
- Accounting review.

---

# 42. ACCOUNTING AND OPERATIONAL RISKS

## Risk 1: Incorrect Accounting Model

Mitigation: Qualified accounting review and testable journal mappings.

## Risk 2: Provider Balance Mismatch

Mitigation: Frequent reconciliation and discrepancy escalation.

## Risk 3: Double Spending

Mitigation: Atomic reservations and concurrency controls.

## Risk 4: Duplicate Financial Posting

Mitigation: Unique accounting event identifiers and idempotency.

## Risk 5: Conversion Settlement Failure

Mitigation: Clearing accounts and durable transaction workflows.

## Risk 6: NGN Liquidity Shortage

Mitigation: Treasury monitoring and payout controls.

## Risk 7: Provider Insolvency

Mitigation: Approved custody and safeguarding arrangements.

## Risk 8: Unauthorized Adjustments

Mitigation: Restricted posting interfaces and maker-checker approval.

## Risk 9: Incomplete Recovery

Mitigation: Durable workflows, immutable journals and reconciliation.

---

# 43. OPEN DECISIONS AND DEPENDENCIES

Before production implementation, the following must be confirmed:

1. The legal custodian of customer stablecoins.
2. The legal ownership and safeguarding treatment of custody assets.
3. The exact Nuvion custody and settlement model.
4. Supported USDC and USDT networks.
5. Asset precision and rounding rules.
6. Provider conversion settlement mechanics.
7. NGN payout provider and settlement account structure.
8. Whether NGN liquidity is prefunded.
9. Refund obligations after failed conversion or payout.
10. Customer fee schedule.
11. Internal transfer fee policy.
12. Revenue recognition policy.
13. Financial reporting currency and valuation policy.
14. Customer asset reconciliation methodology.
15. Treasury account segregation.
16. Required regulatory reporting.
17. Independent accounting approval.

These are implementation dependencies, not permission to bypass the financial integrity requirements defined in this document.

---

# 44. ACCEPTANCE CRITERIA

The ledger architecture will be accepted when:

1. Every financial operation has a defined accounting treatment.
2. Posted journals balance independently by asset.
3. Posted journals are immutable.
4. Customer balances are derived from the ledger.
5. Reservations prevent double spending.
6. Internal transfers are atomic.
7. Deposits cannot be credited twice.
8. Withdrawals cannot execute twice unintentionally.
9. Conversion workflows maintain correct clearing and settlement positions.
10. NGN bank payouts can be reconciled.
11. Failed payments have defined recovery procedures.
12. Refunds and reversals use compensating entries.
13. Provider balances can be reconciled against internal records.
14. Treasury and customer funds are distinguishable.
15. Financial adjustments require authorization.
16. Financial records remain auditable.
17. Automated tests verify the core accounting invariants.
18. Qualified accounting and relevant legal reviews approve the production model.

---

# 45. FINAL LEDGER ARCHITECTURE DEFINITION

DEYPAY will operate an independent double-entry financial ledger implemented in Rust and PostgreSQL.

The ledger will maintain the authoritative internal accounting records for customer stablecoin entitlements, custody assets, settlement positions, fees, revenue, expenses and financial obligations.

USDC and USDT will be represented as separate customer assets.

NGN will be represented for conversion, treasury, settlement and bank payment accounting without creating a general-purpose customer NGN wallet in the initial MVP.

Customer available and reserved balances will be represented through dedicated ledger liability accounts.

Every financial posting must be balanced, immutable, idempotent and auditable.

Internal transfers will move customer entitlements atomically without requiring blockchain execution.

External deposits, withdrawals, conversions and bank payments will use durable workflows and verified settlement evidence.

Nuvion and other infrastructure providers will supply external financial capabilities while DEYPAY retains control of its internal accounting and business logic.

All financial operations must support reconciliation, investigation and recovery.

No customer balance may be changed through an unaccounted database modification.

No external financial operation may be blindly retried after an uncertain outcome.

No posted journal may be deleted to conceal or correct a financial error.

Customer assets and corporate treasury positions must remain distinguishable.

**FINAL FINANCIAL INTEGRITY PRINCIPLE**

Every balance must be explainable.

Every debit must have a corresponding credit.

Every financial obligation must have a recognized accounting basis.

Every external settlement must be verifiable.

Every discrepancy must be investigated.

Every correction must preserve the original financial history.

DEYPAY must always be able to explain where customer value came from, where it moved, where it is held, and whether the corresponding external assets and obligations have been reconciled.

---

**END OF DOCUMENT**

**DEYPAY FINANCIAL LEDGER AND ACCOUNTING DESIGN SPECIFICATION**

**Document ID:** DP-LEDGER-005  
**Version:** 1.0  
**Status:** Proposed Engineering Baseline — Pending Final Validation