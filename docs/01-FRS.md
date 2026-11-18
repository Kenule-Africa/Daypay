# DEYPAY (DP)
# FUNCTIONAL REQUIREMENTS SPECIFICATION (FRS)
## Complete Product, Business, Financial, Technical and Operational Requirements

**Document ID:** DP-001  
**Document Version:** 1.0  
**Document Status:** Baseline Specification – For Product, Engineering, Compliance and Stakeholder Review  
**Product Name:** DEYPAY  
**Product Abbreviation:** DP  
**Initial Market:** Nigeria  
**Initial Platform:** Responsive Web Application / Progressive Web Application (PWA)  
**Backend Technology:** Rust  
**Architecture:** Modular Monolith with Provider Abstraction  
**Primary Stablecoins:** USDC and USDT  
**Preferred Financial Infrastructure Provider:** Nuvion, subject to final commercial, technical and regulatory verification  
**Primary Settlement Currency:** Nigerian Naira (NGN)  
**Document Purpose:** Define the complete functional and non-functional requirements for designing, implementing, testing and launching the DEYPAY MVP.

---

# TABLE OF CONTENTS

1. Executive Summary
2. Product Vision
3. Product Objectives
4. Product Positioning
5. Target Market and User Personas
6. Product Scope
7. Product Principles and Architectural Decisions
8. Account Registration and Authentication
9. Customer Profiles and Identity Verification
10. Wallet Management
11. Stablecoin Deposits
12. Stablecoin Withdrawals
13. Fiat On-Ramp
14. Fiat Off-Ramp
15. Currency Conversion
16. Internal DEYPAY Transfers
17. Pay Anyone – Nigerian Bank Payments
18. External Blockchain Transfers
19. Transaction Management
20. Beneficiary Management
21. Payment Links and QR Codes
22. Notifications
23. Risk Management and Compliance
24. Financial Ledger and Accounting
25. Nuvion Integration Requirements
26. External Provider Integration Architecture
27. Administration and Operations
28. Customer Support and Dispute Management
29. Reporting and Reconciliation
30. Security Requirements
31. Non-Functional Requirements
32. User Experience Requirements
33. System Architecture Requirements
34. Data Management Requirements
35. API Requirements
36. Transaction State Machines
37. Error Handling and Failure Scenarios
38. Business Rules
39. Permissions and Access Control
40. MVP Acceptance Criteria
41. Testing Requirements
42. Deployment and Infrastructure Requirements
43. Monitoring and Observability
44. Backup and Disaster Recovery
45. Regulatory and Legal Dependencies
46. MVP Exclusions
47. Future Product Expansion
48. Implementation Roadmap
49. Project Deliverables
50. Risks and Mitigation Strategies
51. Outstanding Decisions and Dependencies
52. Definition of Done
53. Final Product Definition

---

# 1. EXECUTIVE SUMMARY

DEYPAY is a stablecoin-first digital financial platform designed to make digital dollars and everyday financial transactions accessible through a simple, intuitive and reliable application.

The platform will initially target Nigerian users, including freelancers, remote workers, entrepreneurs, digital asset users, online businesses and individuals who need convenient access to stablecoins and payment services.

DEYPAY will allow users to:

- Register and create accounts.
- Complete identity verification.
- Automatically receive USDC and USDT wallet profiles.
- Deposit USDC and USDT.
- Withdraw USDC and USDT to external blockchain wallets.
- Convert between supported stablecoins and fiat currencies.
- Fund their wallets using supported Nigerian payment methods.
- Convert stablecoins into Naira.
- Send stablecoins to other DEYPAY users.
- Pay any supported Nigerian bank account using stablecoin balances.
- Receive payments from other DEYPAY users.
- Manage beneficiaries.
- Generate and share payment QR codes and payment links.
- Track all transactions.
- Receive notifications.
- Access customer support.

DEYPAY will also provide an administration environment for managing customers, transactions, compliance, financial operations, providers, limits, fees, investigations and reconciliation.

The platform will use USDC and USDT as its primary customer wallet assets.

Naira will primarily be used for funding, conversion, payment settlement and withdrawals. It will not be presented as an ordinary stored-value customer wallet balance in the initial Model B product design.

DEYPAY will maintain its own customer database, wallet records, internal financial ledger, payment engine, transaction history, pricing presentation, business rules and operational controls.

External infrastructure providers, particularly Nuvion, may be used for blockchain wallet provisioning, stablecoin operations, foreign-exchange conversion and payment execution, subject to confirmed capabilities and appropriate legal arrangements.

The initial product will be implemented as a Rust-powered backend with a responsive web application that supports installation as a Progressive Web App.

Development will follow a backend-first approach. The complete financial foundation must be implemented and tested before the primary customer-facing frontend is built.

---

# 2. PRODUCT VISION

DEYPAY's vision is to become a unified financial platform that makes holding, receiving, transferring, converting and spending digital dollars as straightforward as using a traditional banking application.

The product should remove unnecessary complexity associated with blockchain networks, digital asset management and cross-border payments.

A user should not need to understand blockchain infrastructure to use DEYPAY.

The platform should provide a simple experience where users can:

- Hold digital dollars.
- Receive money.
- Send money.
- Pay individuals.
- Pay businesses.
- Convert currencies.
- Access additional financial products over time.

The product philosophy is:

**One wallet. Multiple ways to move money.**

The long-term vision is to expand beyond stablecoin payments into a broader digital financial ecosystem.

---

# 3. PRODUCT OBJECTIVES

## 3.1 Primary Objectives

The MVP must:

1. Provide a secure digital financial account for every registered customer.
2. Support USDC and USDT wallet profiles.
3. Enable compliant stablecoin deposits and withdrawals.
4. Provide reliable currency conversion.
5. Support Nigerian Naira funding and settlement through approved partners.
6. Enable internal user-to-user transfers.
7. Enable stablecoin-funded payments to third-party Nigerian bank accounts.
8. Maintain an accurate, immutable financial ledger.
9. Provide real-time or near-real-time transaction tracking.
10. Provide an operational administration environment.
11. Support transaction monitoring and reconciliation.
12. Provide a scalable foundation for additional financial products.

## 3.2 Business Objectives

DEYPAY should:

- Generate revenue from transaction fees and conversion spreads.
- Reduce friction associated with digital dollar payments.
- Increase customer retention through frequent payment usage.
- Support individuals and businesses.
- Establish partnerships with financial infrastructure providers.
- Create a platform capable of supporting future financial services.

## 3.3 Success Metrics

Initial product metrics should include:

- Registration completion rate.
- KYC completion rate.
- KYC approval rate.
- Wallet provisioning success rate.
- Deposit success rate.
- Withdrawal success rate.
- Internal transfer success rate.
- Pay Anyone success rate.
- Transaction completion time.
- Failed transaction rate.
- Reconciliation discrepancy rate.
- Monthly active users.
- Monthly transacting users.
- Average transaction value.
- Revenue per transaction.
- Customer support resolution time.
- Fraud and suspicious transaction rates.

Targets will be established after partner capabilities, baseline testing and operating costs are confirmed.

---

# 4. PRODUCT POSITIONING

DEYPAY is a customer-facing financial technology platform.

It is not simply a frontend interface for Nuvion.

DEYPAY must own:

- Its product identity.
- Its website and application.
- Its customer relationships.
- Its customer database.
- Its user authentication.
- Its wallet records.
- Its internal ledger.
- Its transaction orchestration.
- Its customer-facing fees and pricing rules.
- Its internal payment logic.
- Its customer support processes.
- Its administration environment.
- Its product roadmap.

Nuvion and other partners will provide selected external infrastructure services.

The customer must experience DEYPAY as one independent product.

Customers should not be required to visit Nuvion's website, create a separate Nuvion account or interact directly with its dashboard.

---

# 5. TARGET MARKET AND USER PERSONAS

## 5.1 Stablecoin User

A freelancer, remote worker, entrepreneur or digital asset user who wants to hold, receive, send and spend digital dollars.

Primary needs:

- USDC and USDT wallets.
- Simple deposits.
- Stablecoin transfers.
- Currency conversion.
- Bank withdrawals.
- Reliable transaction history.

## 5.2 Gift Card Trader

A user who wants to buy or sell supported gift cards and receive settlement into their DEYPAY wallet.

Primary needs:

- Transparent rates.
- Supported gift card categories.
- Clear transaction instructions.
- Reliable verification.
- Fast settlement.
- Dispute resolution.

Gift cards are a future expansion, not a required MVP module.

## 5.3 Prediction Market User

A user who wants to discover and participate in eligible prediction markets.

This requires a separate legally approved integration and is outside the initial MVP.

## 5.4 Stock Investor

A user who wants to access supported stocks and exchange-traded funds through a regulated brokerage partner.

This is a future expansion.

## 5.5 Power User

A customer who uses multiple DEYPAY services and expects one unified application, wallet experience and transaction history.

---

# 6. PRODUCT SCOPE

## 6.1 MVP In-Scope Modules

The MVP must include:

1. Registration and authentication.
2. Customer profiles.
3. Identity verification workflows.
4. Account eligibility and restrictions.
5. USDC and USDT wallet management.
6. Stablecoin deposit monitoring.
7. Stablecoin withdrawal processing.
8. NGN funding and conversion workflows.
9. Stablecoin-to-fiat conversion.
10. Internal DEYPAY transfers.
11. Pay Anyone bank payments.
12. External blockchain transfers.
13. Transaction history.
14. Beneficiary management.
15. QR code functionality.
16. Payment links, subject to risk controls.
17. Notifications.
18. Financial ledger.
19. Risk and compliance workflows.
20. Provider integrations.
21. Administration dashboard.
22. Customer support workflows.
23. Reconciliation.
24. Reporting and audit logs.

## 6.2 Future Expansion

The following are not required for initial MVP acceptance:

- Gift card marketplace.
- Prediction markets.
- Stock trading.
- ETF investment.
- Virtual or physical payment cards.
- Lending.
- Yield products.
- Savings products.
- Business payroll.
- Advanced merchant acquiring.
- Advanced multi-currency accounts.

The architecture should allow these products to be added without redesigning the financial foundation.

---

# 7. PRODUCT PRINCIPLES AND ARCHITECTURAL DECISIONS

The following decisions form the baseline for the product.

## 7.1 Stablecoin-First Model

DEYPAY will use USDC and USDT as its primary customer wallet assets.

Naira is used for funding, conversion, withdrawal and payment settlement.

There will be no ordinary NGN stored-value customer wallet in the initial product.

## 7.2 Registration Before KYC

Users may register and explore the application before completing KYC.

Financial transactions must remain restricted until all required identity, eligibility and risk checks are approved.

## 7.3 Automatic Wallet Provisioning

Every eligible customer should automatically receive USDC and USDT wallet profiles.

Actual blockchain address provisioning may happen asynchronously through the selected custody or infrastructure provider.

The user should not need to manually create each wallet.

## 7.4 Wallet-First Dashboard

The customer dashboard should prioritize:

- Total stablecoin balances.
- USDC balance.
- USDT balance.
- Deposit action.
- Send action.
- Receive action.
- Convert action.
- Pay action.
- Recent transactions.

## 7.5 Three Payment Experiences

The Payments section must provide three distinct options:

1. Pay a DEYPAY user.
2. Pay a Nigerian bank account.
3. Send to an external blockchain wallet.

## 7.6 DEYPAY Owns Its Ledger

All customer balances and financial records must be maintained by DEYPAY's internal financial ledger.

External provider balances must be reconciled against DEYPAY's records.

## 7.7 Provider Independence

External providers must be integrated through internal provider interfaces.

The product must not be designed so that replacing Nuvion requires rebuilding the core wallet, ledger or payment modules.

## 7.8 Compliance by Design

Financial features must be subject to applicable licensing, eligibility, screening, transaction monitoring and risk controls.

---

# 8. ACCOUNT REGISTRATION AND AUTHENTICATION

## 8.1 Registration

The platform must allow a new user to create an account using supported registration credentials.

Initial registration fields:

- First name.
- Last name.
- Email address.
- Phone number.
- Password.
- Country of residence.
- Acceptance of applicable terms and privacy notices.

Additional fields may be requested based on legal and onboarding requirements.

## 8.2 Registration Requirements

FR-REG-001: The system shall allow users to register with a valid email address.

FR-REG-002: The system shall validate email uniqueness.

FR-REG-003: The system shall support phone number verification.

FR-REG-004: The system shall validate phone number uniqueness where required.

FR-REG-005: The system shall securely hash passwords using an approved password hashing algorithm.

FR-REG-006: The system shall send an email verification message.

FR-REG-007: The system shall send an OTP or equivalent phone verification challenge.

FR-REG-008: The system shall prevent financial transactions before required verification and KYC approval.

FR-REG-009: The system shall create an internal immutable customer identifier.

FR-REG-010: The system shall create a customer profile after successful registration.

FR-REG-011: The system shall record acceptance of the applicable terms and privacy policy, including their version.

FR-REG-012: The system shall prevent duplicate registrations according to defined identity and account rules.

## 8.3 Authentication

The system shall support:

- Login.
- Logout.
- Password reset.
- Session management.
- Device management.
- Email verification.
- Phone verification.
- Multi-factor authentication.
- Secure transaction authorization.
- Account recovery.

Passkeys or biometric authentication may be introduced where supported.

Biometric features must not be assumed to be available on every browser or device.

## 8.4 Account Statuses

The system must support:

- REGISTERED.
- CONTACT_UNVERIFIED.
- CONTACT_VERIFIED.
- KYC_REQUIRED.
- KYC_PENDING.
- KYC_APPROVED.
- KYC_REJECTED.
- RESTRICTED.
- SUSPENDED.
- CLOSED.

Account status changes must be auditable.

---

# 9. CUSTOMER PROFILES AND IDENTITY VERIFICATION

## 9.1 Customer Profile

Each customer shall have a DEYPAY-managed profile.

The profile may contain:

- Internal user ID.
- Full name.
- Username.
- Email.
- Verified phone number.
- Date of birth.
- Country.
- Residential address.
- Identity verification status.
- Account status.
- Registration date.
- Risk classification.
- Profile preferences.

## 9.2 KYC Workflow

The platform shall allow customers to submit required identity information.

Depending on approved compliance requirements, this may include:

- Government-issued identification.
- National Identification Number.
- Bank Verification Number.
- Proof of address.
- Selfie or liveness verification.
- Date of birth.
- Residential information.
- Additional source-of-funds information.

The exact data collected must follow applicable laws, data minimization and provider requirements.

## 9.3 KYC Requirements

FR-KYC-001: Users shall be able to initiate KYC after registration.

FR-KYC-002: Users shall be able to upload supported identity documents.

FR-KYC-003: The system shall validate file type and size.

FR-KYC-004: The system shall securely store or transmit identity documents.

FR-KYC-005: The system shall support external identity verification providers.

FR-KYC-006: The system shall receive and process verification results.

FR-KYC-007: The system shall support manual review.

FR-KYC-008: The system shall support rejection reasons.

FR-KYC-009: The system shall allow eligible users to resubmit rejected information.

FR-KYC-010: The system shall prevent financial transactions when KYC approval is required but absent.

FR-KYC-011: The system shall maintain KYC audit records.

FR-KYC-012: The system shall support ongoing review and re-verification.

## 9.4 Nuvion KYC Relationship

DEYPAY must maintain its own customer onboarding experience.

If Nuvion requires customer information for its own compliance checks, DEYPAY may submit the necessary information through Nuvion's API after the appropriate legal basis and consent are established.

Nuvion's entity ID must be stored as an external provider reference.

It must never replace DEYPAY's internal user ID.

DEYPAY and its providers must establish which party performs each identity, AML, sanctions and ongoing monitoring obligation.

---

# 10. WALLET MANAGEMENT

## 10.1 Wallet Model

Every eligible DEYPAY customer shall have:

- A USDC wallet profile.
- A USDT wallet profile.

Wallet profiles shall be created within DEYPAY's database.

The actual custody arrangement and blockchain addresses shall be managed through an approved infrastructure provider.

## 10.2 Wallet Data

Each wallet profile must contain:

- Internal wallet ID.
- Customer ID.
- Asset.
- Network.
- Provider reference.
- Blockchain address, where applicable.
- Wallet status.
- Creation date.
- Provisioning status.
- Deposit availability status.
- Withdrawal availability status.

## 10.3 Wallet Requirements

FR-WAL-001: The system shall automatically create USDC and USDT wallet profiles for eligible users.

FR-WAL-002: The system shall support asynchronous wallet provisioning.

FR-WAL-003: The system shall display wallet provisioning status.

FR-WAL-004: The system shall display deposit addresses once verified and ready.

FR-WAL-005: The system shall display supported blockchain networks.

FR-WAL-006: The system shall prevent deposits to unsupported networks from being presented as supported.

FR-WAL-007: The system shall display available balances.

FR-WAL-008: The system shall distinguish available, pending and reserved balances.

FR-WAL-009: The system shall allow customers to copy wallet addresses.

FR-WAL-010: The system shall support QR representations of deposit addresses.

FR-WAL-011: The system shall display wallet transaction history.

FR-WAL-012: The system shall prevent withdrawals from unavailable or restricted wallet profiles.

## 10.4 Network Strategy

Nuvion's published documentation includes Ethereum, Base, Polygon and Solana for relevant stablecoin operations.

Final network availability must be confirmed for the actual production account.

DEYPAY must select supported networks based on:

- Provider availability.
- Asset support.
- Transaction fees.
- Confirmation time.
- Liquidity.
- Security.
- Customer demand.
- Operational complexity.

The MVP should launch with a limited number of networks rather than enabling every possible network without sufficient operational support.

---

# 11. STABLECOIN DEPOSITS

## 11.1 Deposit Experience

A customer shall be able to select:

1. Receive.
2. Select USDC or USDT.
3. Select a supported network.
4. View the corresponding deposit address.
5. Copy the address or display its QR code.

The customer shall be warned to send only the selected asset on the selected network.

## 11.2 Deposit Processing

The system shall:

1. Detect an incoming blockchain transaction through the provider or blockchain monitoring infrastructure.
2. Receive the relevant event.
3. Verify the provider event and transaction details.
4. Identify the customer wallet.
5. Validate the asset and network.
6. Verify transaction confirmation requirements.
7. Apply required screening.
8. Record the deposit.
9. Credit the customer's internal ledger after the required settlement conditions are met.
10. Notify the customer.

## 11.3 Deposit Requirements

FR-DEP-001: The system shall monitor supported deposit addresses.

FR-DEP-002: The system shall identify incoming transactions.

FR-DEP-003: The system shall store blockchain transaction hashes.

FR-DEP-004: The system shall record asset, network, amount and timestamp.

FR-DEP-005: The system shall track confirmation status.

FR-DEP-006: The system shall prevent duplicate deposit credits.

FR-DEP-007: The system shall credit only after the configured confirmation and risk requirements are satisfied.

FR-DEP-008: The system shall support pending deposits.

FR-DEP-009: The system shall support failed, reversed or disputed deposit investigations where applicable.

FR-DEP-010: The system shall notify customers when deposits become available.

FR-DEP-011: The system shall support reconciliation against provider and blockchain records.

## 11.4 Deposit Statuses

Use the deposit lifecycle in 05-LEDGER-ACCOUNTING.md §25.2. A detected deposit must not automatically become available balance.

---

# 12. STABLECOIN WITHDRAWALS

## 12.1 Withdrawal Experience

A customer shall be able to:

1. Select Send.
2. Select External Wallet.
3. Select USDC or USDT.
4. Select the network.
5. Enter a destination address.
6. Enter an amount.
7. Review the estimated network and service fees.
8. Review the total debit.
9. Confirm the transaction.
10. Complete transaction authentication.

## 12.2 Withdrawal Validation

The system shall validate:

- Customer account status.
- KYC approval.
- Asset availability.
- Network compatibility.
- Address format.
- Available balance.
- Withdrawal limits.
- Transaction risk.
- Screening results.
- Transaction authorization.

## 12.3 Withdrawal Processing

The backend shall:

1. Create a withdrawal request.
2. Validate the request.
3. Obtain an executable quote where applicable.
4. Reserve the authorized principal and applicable customer fees together.
5. Submit the transaction to the approved provider.
6. Record provider references.
7. Track execution.
8. Verify blockchain transaction status.
9. Finalize the ledger entries.
10. Release reservations only when a confirmed failure or cancellation permits it.

## 12.4 Withdrawal Requirements

FR-WDR-001: Customers shall be able to withdraw supported assets.

FR-WDR-002: Customers shall be able to select supported networks.

FR-WDR-003: Customers shall be able to enter destination addresses.

FR-WDR-004: The system shall validate address and network compatibility.

FR-WDR-005: The system shall display transaction fees.

FR-WDR-006: The system shall require explicit transaction confirmation.

FR-WDR-007: The system shall support withdrawal limits.

FR-WDR-008: The system shall reserve funds before execution.

FR-WDR-009: The system shall track provider execution status.

FR-WDR-010: The system shall store blockchain transaction references.

FR-WDR-011: The system shall prevent duplicate submissions.

FR-WDR-012: The system shall support manual investigation.

FR-WDR-013: The system shall release reservations only after a confirmed failure or approved cancellation.

---

# 13. FIAT ON-RAMP

## 13.1 Definition

On-ramp means converting traditional money into digital assets.

For DEYPAY, this means a customer funds a purchase using Naira and receives USDC or USDT.

## 13.2 Customer Journey

1. Customer selects Add Money.
2. Customer selects Naira funding.
3. Customer enters the amount of NGN to spend.
4. DEYPAY retrieves an executable quote.
5. The application displays:
   - NGN amount.
   - Selected stablecoin.
   - Exchange rate.
   - Fees.
   - Expected stablecoin amount.
   - Quote expiry.
6. Customer confirms.
7. DEYPAY generates the supported funding instructions.
8. Customer sends Naira through the approved payment channel.
9. The payment provider confirms settlement.
10. DEYPAY validates the payment.
11. The stablecoin conversion is executed.
12. DEYPAY credits the customer's stablecoin ledger balance.
13. Customer receives confirmation.

## 13.3 Requirements

FR-ONR-001: The system shall support NGN funding through approved payment partners.

FR-ONR-002: The system shall generate executable quotes.

FR-ONR-003: The system shall display exchange rates and applicable fees.

FR-ONR-004: The system shall support quote expiry.

FR-ONR-005: The system shall track incoming NGN payment status.

FR-ONR-006: The system shall prevent crediting before required payment and settlement confirmation.

FR-ONR-007: The system shall execute the approved stablecoin conversion.

FR-ONR-008: The system shall record all provider references.

FR-ONR-009: The system shall credit the correct customer wallet.

FR-ONR-010: The system shall support payment investigation and reconciliation.

## 13.4 Important Business Rule

DEYPAY must not treat an unconfirmed bank transfer screenshot or customer-submitted receipt as proof of settlement.

A trusted provider confirmation or verified settlement record is required.

Give each funding order a customer-visible reference, expected NGN amount, expiry and status. Investigate late, unmatched or mismatched collections; credit stablecoins once after verified collection and delivery.

---

# 14. FIAT OFF-RAMP

## 14.1 Definition

Off-ramp means converting digital assets into traditional money.

For DEYPAY, this means converting USDC or USDT into NGN and paying the customer through a supported bank account.

## 14.2 Customer Journey

1. Customer selects Withdraw to Bank.
2. Customer selects USDC or USDT.
3. Customer selects a beneficiary verified as the customer’s own permitted bank account.
4. Customer enters the intended NGN amount or stablecoin amount.
5. DEYPAY retrieves an executable conversion quote.
6. The customer reviews the rate, fees and payout amount.
7. Customer authorizes the withdrawal.
8. DEYPAY reserves the stablecoin.
9. Conversion is executed.
10. The payout provider sends NGN.
11. DEYPAY verifies payout status.
12. DEYPAY finalizes accounting.
13. Customer receives confirmation.

## 14.3 Requirements

FR-OFF-001: Customers shall be able to withdraw to supported Nigerian bank accounts.

FR-OFF-002: Customers shall be able to select a verified beneficiary.

FR-OFF-003: The system shall support NGN-first and stablecoin-first input.

FR-OFF-004: The system shall retrieve executable quotes.

FR-OFF-005: The system shall disclose all fees.

FR-OFF-006: The system shall reserve the required stablecoin amount.

FR-OFF-007: The system shall execute approved conversion and payout workflows.

FR-OFF-008: The system shall verify payout status.

FR-OFF-009: The system shall finalize or release ledger reservations based on confirmed outcomes.

FR-OFF-010: The system shall support payout failure investigation.

---

# 15. CURRENCY CONVERSION

## 15.1 Supported Conversions

The initial product shall prioritize:

- USDC to USDT.
- USDT to USDC.
- USDC to NGN through supported settlement arrangements.
- USDT to NGN through supported settlement arrangements.
- NGN to USDC through supported funding arrangements.
- NGN to USDT through supported funding arrangements.

Actual available conversion pairs depend on approved provider corridors.

## 15.2 Quote Requirements

Every executable quote must include:

- Quote ID.
- Source asset.
- Destination asset.
- Input amount.
- Output amount.
- Exchange rate.
- Fees.
- Net amount.
- Expiry timestamp.
- Provider reference.
- Quote status.

## 15.3 Conversion Requirements

FR-FX-001: The system shall retrieve executable quotes from approved providers.

FR-FX-002: The system shall display transparent pricing.

FR-FX-003: The system shall enforce quote expiry.

FR-FX-004: The system shall prevent execution against expired quotes.

FR-FX-005: The system shall verify the quote before execution.

FR-FX-006: The system shall record the exact rate applied.

FR-FX-007: The system shall record fees separately.

FR-FX-008: The system shall reconcile executed conversions.

FR-FX-009: The system shall support provider errors and expired quotes.

FR-FX-010: The system shall prevent duplicate conversion execution.

---

# 16. INTERNAL DEYPAY TRANSFERS

## 16.1 Overview

DEYPAY shall support instant internal transfers between eligible DEYPAY customers.

Users must be able to identify recipients using:

- Unique username.
- Verified phone number, where the recipient has enabled discoverability.
- QR code.
- Saved DEYPAY beneficiary.
- Supported payment link.

## 16.2 Username System

Every customer shall have a unique username.

Examples:

- @daniel.
- @michael.
- @rexzy.

The username shall be mapped to the customer's internal DEYPAY user ID.

A username must never be treated as a blockchain wallet address.

Users may change usernames subject to availability, abuse controls and policy.

## 16.3 Phone Number Discovery

Customers may optionally allow other users to find them using their verified phone number.

Phone discoverability must be configurable.

DEYPAY must protect phone numbers from unauthorized exposure.

## 16.4 Transfer Journey

1. Sender selects Pay DEYPAY User.
2. Sender enters a username or supported phone number.
3. DEYPAY resolves the recipient.
4. The system displays the recipient's permitted identity details.
5. Sender selects USDC or USDT.
6. Sender enters an amount.
7. The system displays the transaction details.
8. Sender authorizes the transaction.
9. DEYPAY checks balance, account status, limits and risk.
10. The ledger debits the sender and credits the recipient.
11. Both customers receive notifications.

## 16.5 Requirements

FR-INT-001: The system shall support unique usernames.

FR-INT-002: The system shall resolve usernames to internal customer IDs.

FR-INT-003: The system shall support optional phone discoverability.

FR-INT-004: The system shall support QR-based internal payments.

FR-INT-005: The system shall support saved DEYPAY beneficiaries.

FR-INT-006: The system shall validate recipient eligibility.

FR-INT-007: The system shall prevent sending to the same user unless explicitly permitted by product policy.

FR-INT-008: The system shall validate available balance.

FR-INT-009: The system shall apply transaction limits.

FR-INT-010: The system shall authorize transfers.

FR-INT-011: The system shall debit and credit through an atomic ledger operation.

FR-INT-012: The system shall prevent duplicate transfers.

FR-INT-013: The system shall notify both parties.

FR-INT-014: The system shall record immutable transfer references.

Both participants can see the same transfer reference in their own history and receipts. Only their permitted counterparty details are visible.

## 16.6 Internal Transfer Accounting

Internal transfers must not create unnecessary blockchain transactions.

They should be recorded as balanced internal ledger movements.

However, the product's regulatory treatment must be approved before this functionality is enabled for customers.

---

# 17. PAY ANYONE – NIGERIAN BANK PAYMENTS

## 17.1 Overview

Pay Anyone is a required MVP feature. Implement and test it with mock or sandbox infrastructure. Real-money activation depends on the specific route approval in §45.3.

It allows customers to spend their USDC or USDT to pay a third-party Nigerian bank account.

The recipient does not need:

- A DEYPAY account.
- A cryptocurrency wallet.
- USDC.
- USDT.
- Knowledge of blockchain technology.

The recipient receives ordinary NGN through a supported bank transfer.

Example:

A customer wants to pay a grocery store ₦50,000.

The customer funds the payment using USDT or USDC.

DEYPAY converts the required stablecoin amount into Naira through an approved arrangement and instructs the Nigerian payout partner to send ₦50,000 to the store.

## 17.2 Recipient Account Verification

The customer shall enter:

- Bank.
- Account number.

DEYPAY shall request account resolution from an approved provider.

The provider should return:

- Resolved account name.
- Bank.
- Account number or masked account reference.
- Verification result.

The user must review and confirm the recipient before payment.

The system must not rely on a recipient name manually typed by the sender.

## 17.3 Payment Amount

The default payment input must be NGN-first.

The customer should be able to enter:

₦50,000.

DEYPAY calculates the required USDC or USDT.

The customer may also choose stablecoin-first input.

For example:

20 USDT → calculated NGN payout.

## 17.4 Quote Presentation

The confirmation screen must display:

- Recipient name.
- Bank.
- Account number, appropriately masked.
- Recipient amount.
- Funding asset.
- Stablecoin amount.
- Exchange rate.
- Platform fee.
- Provider fee, where applicable.
- Total debit.
- Quote expiry.
- Expected processing time, where reliably available.

## 17.5 Payment Processing

The backend shall:

1. Validate the sender.
2. Validate recipient details.
3. Retrieve an executable quote.
4. Validate quote expiry.
5. Check transaction limits.
6. Perform risk screening.
7. Authorize the transaction.
8. Reserve customer stablecoin funds.
9. Execute the approved conversion/liquidity arrangement.
10. Submit the NGN payout instruction.
11. Track provider status.
12. Verify the payout result against settlement evidence. After confirmed conversion, track the NGN obligation until payout or approved recovery settles it.
13. Finalize accounting.
14. Notify the sender.
15. Generate a receipt.

## 17.6 Requirements

FR-PAY-001: The system shall support Nigerian bank selection.

FR-PAY-002: The system shall support bank account number entry.

FR-PAY-003: The system shall support bank account name resolution through an approved provider.

FR-PAY-004: The system shall display resolved recipient details.

FR-PAY-005: The system shall require recipient confirmation.

FR-PAY-006: The system shall support NGN-first payment entry.

FR-PAY-007: The system shall support stablecoin-first payment entry.

FR-PAY-008: The system shall retrieve an executable conversion quote.

FR-PAY-009: The system shall display transparent fees.

FR-PAY-010: The system shall reserve customer funds.

FR-PAY-011: The system shall submit approved payouts.

FR-PAY-012: The system shall track payout status.

FR-PAY-013: The system shall prevent duplicate payouts.

FR-PAY-014: The system shall support uncertain payout states.

FR-PAY-015: The system shall generate payment receipts.

FR-PAY-016: The system shall maintain payment history.

FR-PAY-017: The system shall support payout investigations.

FR-PAY-018: The system shall reconcile payout instructions with provider settlement records.

## 17.7 Pay Anyone Liquidity Requirement

The product must not assume that conversion and payout happen automatically without available liquidity.

The operating model must establish:

- Who provides stablecoin liquidity.
- Who provides NGN settlement funds.
- Whether NGN payout accounts are prefunded.
- Who manages settlement pool replenishment.
- How failed payments are handled.
- How the financial exposure is measured.
- How DEYPAY reconciles the settlement pool.

The exact liquidity model is subject to the approved provider arrangement.

---

# 18. EXTERNAL BLOCKCHAIN TRANSFERS

DEYPAY shall allow eligible customers to send supported stablecoins to external blockchain addresses.

This feature is distinct from internal DEYPAY transfers.

The customer must select:

- Asset.
- Network.
- Destination address.
- Amount.

The system must:

- Validate the address.
- Verify asset/network compatibility.
- Display fees.
- Enforce limits.
- Perform risk screening.
- Require transaction authorization.
- Reserve funds.
- Execute through the approved provider.
- Track blockchain status.
- Store the transaction hash.
- Finalize ledger accounting.

The application must clearly warn users that blockchain transfers may be irreversible.

---

# 19. TRANSACTION MANAGEMENT

DEYPAY must provide one unified transaction history.

Supported categories:

- Deposits.
- Withdrawals.
- Internal transfers.
- Incoming internal transfers.
- External blockchain sends.
- Bank payments.
- Bank withdrawals.
- Conversions.
- Adjustments.
- Reversals.
- Refunds, where supported.

Each transaction record must include:

- Internal transaction ID.
- Reference.
- Customer ID.
- Transaction type.
- Asset.
- Network, where applicable.
- Amount.
- Fees.
- Exchange rate, where applicable.
- Recipient or sender.
- Provider reference.
- Blockchain hash, where applicable.
- Creation time.
- Updated time.
- Status.

## Requirements

FR-TXN-001: Users shall view their transaction history.

FR-TXN-002: Users shall filter transactions by type.

FR-TXN-003: Users shall filter by date.

FR-TXN-004: Users shall search by reference.

FR-TXN-005: Users shall open transaction details.

FR-TXN-006: Users shall view transaction status.

FR-TXN-007: Users shall access downloadable or shareable receipts where appropriate.

FR-TXN-008: Users shall receive updates when transaction statuses change.

FR-TXN-009: Users shall not be able to modify completed financial transaction records.

FR-TXN-010: The system shall maintain complete transaction audit history.

---

# 20. BENEFICIARY MANAGEMENT

The platform shall support two beneficiary categories:

1. DEYPAY users.
2. External Nigerian bank accounts.

Customers shall be able to:

- Add beneficiaries.
- Verify beneficiaries.
- Save beneficiaries.
- Rename saved beneficiaries.
- Delete beneficiaries.
- View previous payment activity.

The system shall enforce appropriate beneficiary verification and risk controls.

Changes to beneficiaries may require additional authentication.

Renaming changes the owner’s private label only. A saved DEYPAY recipient retains the same immutable customer ID after a username change.

---

# 21. PAYMENT LINKS AND QR CODES

## 21.1 QR Codes

DEYPAY shall support:

- Personal receive QR codes.
- Wallet deposit QR codes.
- Internal payment QR codes.
- Supported payment request QR codes.

QR codes must contain only the information necessary for the intended operation.

## 21.2 Payment Links

Payment links are part of MVP delivery, with customer availability subject to the approved payment-link risk controls in §6.1.

Links should have:

- Unique identifiers.
- Expiry.
- Payment amount, where applicable.
- Recipient identity.
- Payment status.
- Appropriate abuse and fraud controls.

A payment link must not allow unauthorized access to customer accounts.

---

# 22. NOTIFICATIONS

The system shall support notifications through:

- In-app notifications.
- Email.
- Push notifications, where supported.
- SMS or other verified messaging channels where required.

Events include:

- Registration.
- Email verification.
- Phone verification.
- KYC submission.
- KYC approval.
- KYC rejection.
- Wallet provisioning.
- Deposit detected.
- Deposit credited.
- Withdrawal submitted.
- Withdrawal completed.
- Internal transfer sent.
- Internal transfer received.
- Bank payout initiated.
- Bank payout completed.
- Bank payout failed.
- Conversion completed.
- Security alerts.
- Account restrictions.

Notifications must not expose unnecessary sensitive financial or personal information.

The backend must process transaction notifications independently of whether the PWA is open.

---

# 23. RISK MANAGEMENT AND COMPLIANCE

DEYPAY must incorporate compliance controls into all financial workflows.

## 23.1 Customer Screening

The system should support:

- Identity verification.
- Sanctions screening.
- Politically exposed person screening.
- Adverse media checks where required.
- Customer risk classification.
- Ongoing customer monitoring.

## 23.2 Transaction Screening

Transactions must be evaluated against applicable:

- Customer limits.
- Velocity limits.
- Sanctions rules.
- Suspicious activity indicators.
- Wallet/address risk.
- Transaction patterns.
- Geographic restrictions.
- Source-of-funds requirements.

## 23.3 Risk Controls

The system must support:

- Daily limits.
- Monthly limits.
- Per-transaction limits.
- Withdrawal limits.
- Internal transfer limits.
- Bank payment limits.
- Account restrictions.
- Manual review.
- Temporary holds.
- Transaction blocking.

Limits must be configurable by authorized administrators.

## 23.4 Suspicious Transactions

The system must allow authorized compliance personnel to:

- View suspicious transaction alerts.
- Investigate customers.
- Review transaction histories.
- Add investigation notes.
- Escalate cases.
- Restrict accounts where legally justified.
- Record investigation outcomes.

Regulatory reporting obligations must be assigned to the legally responsible entity.

---

# 24. FINANCIAL LEDGER AND ACCOUNTING

This is one of the most important components of DEYPAY.

The financial ledger must be implemented as a first-class backend module.

It must not be treated as a simple balance field in the customer database.

## 24.1 Double-Entry Accounting

Every financial movement must create balanced accounting entries.

For every posted journal:

Total debits = Total credits.

Entries must be immutable after posting.

Corrections must be made through compensating entries rather than editing or deleting historical financial records.

## 24.2 Ledger Account Types

The ledger should support:

- Customer available balances.
- Customer reserved balances.
- Customer pending balances.
- Provider asset accounts.
- Custody asset accounts.
- Settlement accounts.
- Fee revenue accounts.
- Conversion clearing accounts.
- Suspense accounts.
- Reversal accounts.
- Operational adjustment accounts.

## 24.3 Balance Categories

The system must distinguish:

Available balance: Funds eligible for new transactions.

Reserved balance: Funds committed to an in-progress transaction.

Pending balance: Funds detected or expected but not yet available.

Settled balance: Funds whose settlement conditions have been satisfied.

These categories must be defined consistently in the ledger and exposed appropriately through the API.

## 24.4 Ledger Requirements

FR-LED-001: All financial operations shall be recorded in a double-entry ledger.

FR-LED-002: Every posted journal shall balance.

FR-LED-003: Posted journal entries shall be immutable.

FR-LED-004: Financial corrections shall use compensating entries.

FR-LED-005: The ledger shall support multiple assets.

FR-LED-006: The ledger shall distinguish available, pending and reserved funds.

FR-LED-007: The ledger shall support atomic financial operations.

FR-LED-008: The ledger shall prevent negative available balances unless explicitly supported by an approved accounting arrangement.

FR-LED-009: The ledger shall support transaction references.

FR-LED-010: The ledger shall support financial reconciliation.

FR-LED-011: Every adjustment shall include an authorized actor and reason.

FR-LED-012: The system shall maintain a complete audit trail.

## 24.5 Balance Calculation

Derive available and reserved balances from their respective posted customer liability accounts. Reservation records identify the operation and lifecycle. Pending deposit amounts are displayed separately until eligible for credit.

The system may maintain optimized balance projections for performance.

However, projections must always be reconstructable from the underlying ledger.

The ledger remains the financial source of truth.

## 24.6 Internal Transfer Example

Daniel sends Michael 20 USDT.

The ledger records:

Debit: Daniel's available USDT liability – 20 USDT.

Credit: Michael's available USDT liability – 20 USDT.

The transaction is recorded as one atomic internal transfer.

## 24.7 Pay Anyone Example

A customer pays ₦50,000 using USDT.

The system must account for:

- Customer stablecoin liability.
- Customer reserved balance.
- Stablecoin conversion.
- Provider settlement.
- NGN payout.
- Fees.
- Any exchange or liquidity differences.

Exact journal entries depend on the legal and settlement structure.

Finance and engineering must approve the accounting mapping before production.

## 24.8 Financial Reconciliation

DEYPAY must reconcile:

- Internal customer liabilities.
- Stablecoin assets held through custody providers.
- External wallet balances.
- NGN settlement accounts.
- Liquidity provider positions.
- Payment provider balances.
- Fees.
- Pending transactions.
- Failed and reversed transactions.

Unexplained discrepancies must generate alerts.

---

# 25. NUVION INTEGRATION REQUIREMENTS

## 25.1 Integration Strategy

Nuvion shall be treated as a preferred external financial infrastructure provider.

It shall not own DEYPAY's customer application, internal ledger, customer database or core payment business logic.

Nuvion integration must be implemented inside DEYPAY's provider integration layer.

## 25.2 Expected Nuvion Capabilities

Subject to production confirmation, DEYPAY intends to use Nuvion for:

- Customer/entity provisioning.
- Stablecoin account creation.
- USDC wallet provisioning.
- USDT wallet provisioning.
- Blockchain deposit infrastructure.
- External stablecoin transfer execution.
- FX quotes.
- Stablecoin conversion.
- Fiat payout execution.
- Transaction status retrieval.
- Provider webhook notifications.

## 25.3 Nuvion API Client

The backend shall implement a dedicated Nuvion API client.

It must handle:

- Authentication.
- Request signing or authorization requirements.
- HTTP requests.
- Response parsing.
- Error handling.
- Timeout handling.
- Retry policy.
- Rate limits.
- Provider references.
- Request logging with sensitive data redacted.

## 25.4 Nuvion Provider Modules

The integration shall be logically divided into:

1. NuvionEntityProvider.
2. NuvionWalletProvider.
3. NuvionDepositProvider.
4. NuvionTransferProvider.
5. NuvionFXProvider.
6. NuvionPayoutProvider.
7. NuvionWebhookHandler.
8. NuvionReconciliationProvider.

## 25.5 External Reference Mapping

DEYPAY must store:

- Internal customer ID.
- Nuvion entity ID.
- Internal wallet ID.
- Nuvion account ID.
- Internal transaction ID.
- Nuvion transaction ID.
- Provider event ID.
- Blockchain transaction hash.

External references must never replace internal DEYPAY identifiers.

## 25.6 Webhooks

The integration must support:

- Signature verification.
- Event validation.
- Event deduplication.
- Persistent event storage.
- Asynchronous processing.
- Event status tracking.
- Failed event recovery.
- Reconciliation against provider APIs.

Webhook processing must be idempotent.

An event must never credit the same deposit twice.

A webhook is a notification of an event, not automatically proof that a financial operation has settled.

## 25.7 Nuvion Production Dependencies

The following must be confirmed before production use:

- Nigerian business onboarding eligibility.
- Nigerian customer eligibility.
- USDC/NGN support.
- USDT/NGN support.
- Nigerian bank payout capability.
- Nigerian bank account name resolution.
- Third-party bank payouts.
- Applicable custody arrangements.
- Supported networks.
- API production credentials.
- Commercial fees.
- Transaction limits.
- Liquidity arrangements.
- Settlement times.
- Legal entities and regulated partners.
- Contractual responsibilities.
- Applicable Nigerian regulatory permissions.

For each proposed NGN route, confirm the contracting entities, funding and settlement accounts, permitted recipient types, and responsibilities for execution, reconciliation, safeguarding and refunds. Record own-bank and third-party recipient permissions separately.

## 25.8 Provider Independence

DEYPAY must define provider-neutral interfaces.

The rest of the application should call DEYPAY services rather than directly calling Nuvion.

Example:

DEYPAY Wallet Service → Wallet Provider Interface → Nuvion Adapter.

If another provider is introduced, the alternative provider must implement the same interface.

This will allow DEYPAY to replace or supplement Nuvion without rebuilding the core platform.

---

# 26. EXTERNAL PROVIDER INTEGRATION ARCHITECTURE

DEYPAY must support multiple external provider categories.

## 26.1 Provider Categories

- Stablecoin infrastructure.
- Custody and wallet security.
- Blockchain infrastructure.
- NGN collection.
- Nigerian bank payouts.
- Liquidity and conversion.
- KYC and identity verification.
- AML and sanctions screening.
- Email and messaging.
- Analytics and monitoring.

## 26.2 Provider Requirements

Every provider integration must:

- Use secure authentication.
- Store credentials securely.
- Use provider-specific adapters.
- Store external references.
- Support timeouts.
- Support safe retries.
- Support idempotency where available.
- Verify webhooks.
- Provide transaction status retrieval.
- Support reconciliation.
- Generate operational alerts.
- Avoid exposing provider-specific errors directly to customers.

## 26.3 Provider Failure

A provider outage must not corrupt DEYPAY's ledger.

Protect the outstanding balance or obligation for the current financial stage (§36.5) and reconcile uncertain transactions before finalization.

---

# 27. ADMINISTRATION AND OPERATIONS

DEYPAY must provide a secure administrative environment.

## 27.1 Admin Dashboard

The dashboard should provide:

- Total registered customers.
- Verified customers.
- Pending KYC cases.
- Restricted accounts.
- Total customer liabilities by asset.
- Deposit volume.
- Withdrawal volume.
- Internal transfer volume.
- Bank payout volume.
- Conversion volume.
- Pending transactions.
- Failed transactions.
- Provider health.
- Reconciliation alerts.
- Revenue summaries.

## 27.2 Customer Management

Authorized administrators shall be able to:

- Search customers.
- View customer profiles.
- View KYC status.
- View wallet profiles.
- View transaction histories.
- View risk indicators.
- Apply authorized restrictions.
- Review account status.
- Add internal notes.

## 27.3 KYC Management

Authorized reviewers shall be able to:

- View pending submissions.
- Review identity documents.
- Approve eligible submissions.
- Reject submissions with reasons.
- Request additional information.
- Escalate cases.

Record each reviewer decision, reason and resulting case status in the audit log.

## 27.4 Transaction Management

Authorized administrators shall be able to:

- Search transactions.
- Filter by status.
- View transaction details.
- View provider references.
- Review failed transactions.
- Investigate uncertain transactions.
- Review blockchain references.
- Record investigation notes.
- Escalate issues.

## 27.5 Financial Administration

The system shall support:

- Ledger inspection.
- Reconciliation.
- Suspense account review.
- Fee configuration.
- Transaction limit configuration.
- Provider status monitoring.
- Approved financial adjustments.

Financial adjustments must require appropriate authorization and audit records.

## 27.6 Maker-Checker

High-risk administrative actions must support maker-checker approval.

One administrator initiates an action.

A different authorized administrator approves it.

This applies to actions such as:

- Manual financial adjustments.
- High-risk reversals.
- Certain account restrictions.
- Sensitive configuration changes.
- High-value operational interventions.

An administrator must not approve their own restricted action.

---

# 28. CUSTOMER SUPPORT AND DISPUTE MANAGEMENT

The platform must support customer service operations.

Customers should be able to:

- Submit support requests.
- Select a transaction.
- Explain an issue.
- Attach permitted supporting information.
- Receive a support reference.
- Track support status.

Support personnel should be able to:

- View customer history.
- Review transaction status.
- View provider references.
- Add internal notes.
- Escalate cases.
- Update case status.
- Communicate with customers.

Support staff must not have unrestricted financial adjustment permissions.

---

# 29. REPORTING AND RECONCILIATION

## 29.1 Reports

The platform should generate:

- Customer reports.
- Wallet reports.
- Deposit reports.
- Withdrawal reports.
- Internal transfer reports.
- Pay Anyone reports.
- Conversion reports.
- Fee reports.
- Provider reports.
- Reconciliation reports.
- Risk reports.
- Audit reports.

## 29.2 Reconciliation

Reconciliation must compare DEYPAY records against external provider data.

The system shall identify:

- Missing provider transactions.
- Missing internal transactions.
- Duplicate transactions.
- Amount mismatches.
- Fee mismatches.
- Unexpected balances.
- Transactions stuck in processing.
- Provider settlement discrepancies.

## 29.3 Reconciliation Frequency

The system should support:

- Event-driven reconciliation.
- Scheduled reconciliation.
- Daily financial reconciliation.
- Manual investigation and replay where supported.

All discrepancies must have statuses and assigned owners.

---

# 30. SECURITY REQUIREMENTS

Security is a foundational requirement.

## 30.1 Authentication Security

- Secure password hashing.
- Multi-factor authentication.
- Session expiry.
- Device/session management.
- Login rate limiting.
- Brute-force protection.
- Secure password reset.
- Suspicious login detection.

## 30.2 API Security

- HTTPS.
- Secure authentication.
- Authorization checks.
- Request validation.
- Rate limiting.
- Input sanitization.
- Secure error handling.
- API access logging.
- Protection against replay attacks where relevant.

## 30.3 Financial Security

- Transaction authorization.
- Idempotency.
- Double-entry ledger.
- Balance reservations.
- Immutable transaction records.
- Provider verification.
- Webhook signature verification.
- Administrative access control.
- Financial reconciliation.

## 30.4 Secrets Management

Provider credentials, API keys, signing secrets and sensitive cryptographic materials must be stored in an approved secrets-management system.

They must never be committed to source control or exposed in application logs.

## 30.5 Data Security

- Encryption in transit.
- Encryption at rest.
- Restricted access to identity documents.
- Sensitive field protection.
- Audit logging.
- Secure backups.
- Data retention policies.

---

# 31. NON-FUNCTIONAL REQUIREMENTS

## 31.1 Performance

The application should be responsive under expected MVP traffic.

Initial engineering targets:

- Typical internal API response time: under 500 ms at the 95th percentile for non-provider-dependent requests.
- Internal ledger operations: under 500 ms at the 95th percentile under the defined initial load.
- Dashboard initial data response: under 2 seconds under normal conditions.
- Provider-dependent operations: measured separately because external networks introduce variable latency.

These are engineering targets, not guarantees of external settlement times.

## 31.2 Availability

The initial target should be 99.9% monthly availability for core backend services, excluding agreed maintenance and external provider outages.

## 31.3 Scalability

The architecture must support growth in:

- Registered users.
- Active users.
- Wallets.
- Transactions.
- Provider integrations.
- Payment volume.
- Administrative operations.

## 31.4 Reliability

The platform must support:

- Safe retries.
- Idempotent operations.
- Persistent event processing.
- Transaction recovery.
- Reconciliation.
- Database backups.
- Disaster recovery.

## 31.5 Maintainability

The backend must use clearly separated modules, typed interfaces, documented APIs and automated tests.

## 31.6 Observability

The system must provide:

- Structured logs.
- Metrics.
- Distributed traces where applicable.
- Error monitoring.
- Provider health checks.
- Financial operation alerts.

## 31.7 Accessibility

The frontend should follow practical accessibility principles, including keyboard navigation, readable contrast, labels and responsive layouts.

---

# 32. USER EXPERIENCE REQUIREMENTS

## 32.1 General Experience

The application must be:

- Mobile-first.
- Responsive.
- Simple.
- Fast.
- Secure.
- Consistent.
- Accessible.
- Easy to understand.

## 32.2 Dashboard

The dashboard should display:

- Total stablecoin portfolio value.
- USDC balance.
- USDT balance.
- Deposit.
- Send.
- Receive.
- Convert.
- Pay.
- Recent transactions.

Portfolio values may include an NGN equivalent for informational purposes.

That display must not imply that the customer holds an NGN wallet balance.

## 32.3 Navigation

Suggested primary navigation:

- Home.
- Wallet.
- Payments.
- Activity.
- Profile.

The Payments section must provide:

- Pay DEYPAY User.
- Pay Bank Account.
- Send to External Wallet.

## 32.4 KYC Experience

The application should clearly communicate:

- Whether KYC is required.
- What information is needed.
- Current verification status.
- Why a financial feature is unavailable.
- How to resolve rejected or incomplete submissions.

## 32.5 PWA Requirements

The application shall support installation on compatible devices.

It should support:

- Responsive layouts.
- Secure sessions.
- QR scanning where available.
- Push notifications where supported.
- Shareable links.
- Appropriate offline messaging.

Financial transactions must not depend on the browser remaining open.

Backend transaction processing must continue independently.

---

# 33. SYSTEM ARCHITECTURE REQUIREMENTS

## 33.1 Architectural Style

DEYPAY shall initially use a modular monolith.

A modular monolith is one backend application divided into clearly separated business modules.

It avoids unnecessary early microservice complexity while preserving boundaries that can support future extraction.

## 33.2 Proposed Backend Technology

- Rust.
- Axum.
- Tokio.
- PostgreSQL.
- SQLx.
- Redis.
- OpenAPI.
- OpenTelemetry.
- Docker.
- Compatible cloud infrastructure.

A durable asynchronous job/event processing mechanism must be selected.

The final choice between NATS, RabbitMQ or a PostgreSQL-backed queue must be based on actual operational requirements rather than assumed complexity.

## 33.3 Backend Modules

1. Identity.
2. Authentication.
3. Customer profiles.
4. KYC.
5. Wallet management.
6. Financial ledger.
7. Deposits.
8. Withdrawals.
9. FX and conversion.
10. Payments.
11. Provider integration.
12. Risk and compliance.
13. Notifications.
14. Administration.
15. Reporting.
16. Reconciliation.

## 33.4 Architecture Principle

The frontend communicates only with DEYPAY APIs.

DEYPAY backend services communicate with external providers.

The frontend must never directly call Nuvion using privileged credentials.

---

# 34. DATA MANAGEMENT REQUIREMENTS

## 34.1 Core Data Entities

The database should include logically separated records for:

- Users.
- User profiles.
- Authentication credentials.
- Sessions.
- Devices.
- Usernames.
- KYC submissions.
- KYC verification results.
- Wallets.
- Wallet addresses.
- Assets.
- Networks.
- Ledger accounts.
- Ledger journals.
- Ledger entries.
- Balance projections.
- Reservations.
- Transactions.
- Deposits.
- Withdrawals.
- Transfers.
- Payouts.
- FX quotes.
- Conversions.
- Beneficiaries.
- Payment links.
- Notifications.
- Provider accounts.
- Provider events.
- Provider transactions.
- Compliance cases.
- Risk alerts.
- Support tickets.
- Administrative users.
- Roles.
- Permissions.
- Audit logs.
- Reconciliation records.

## 34.2 Identifier Strategy

Internal identifiers should use stable, non-sequential public identifiers where appropriate.

Database relationships must use appropriate primary and foreign keys.

Provider identifiers must be stored separately from internal identifiers.

## 34.3 Financial Precision

Financial amounts must use precise decimal or integer base-unit representations.

Binary floating-point arithmetic must never be used for financial balances or transaction calculations.

Asset precision must be explicitly defined.

Stablecoin token precision and provider API amount precision must be handled independently.

## 34.4 Data Retention

Retention policies must reflect applicable financial, legal, privacy and regulatory requirements.

Sensitive information must not be retained indefinitely without a legal or operational basis.

---

# 35. API REQUIREMENTS

The backend must expose versioned REST APIs.

Canonical routes and public statuses are owned by 04-API-SPECIFICATION.md. Paths below are relative to:

/api/v1

## 35.1 Authentication

POST /auth/register

POST /auth/login

POST /auth/logout

POST /auth/refresh

POST /auth/verify-email

POST /auth/verify-phone

POST /auth/forgot-password

POST /auth/reset-password

## 35.2 Customer

GET /me

PATCH /me

GET /me/status

## 35.3 KYC

POST /kyc/start

POST /kyc/details

POST /kyc/documents/upload-request

POST /kyc/submit

GET /kyc/status

## 35.4 Wallets

GET /wallets

GET /wallets/{wallet_id}

GET /wallets/{wallet_id}/balance

Provision eligible wallets asynchronously under §10.

## 35.5 Deposits

GET /deposits

GET /deposits/{deposit_id}

POST /wallets/{wallet_id}/deposit-addresses

## 35.6 Withdrawals

POST /withdrawals/quotes

POST /withdrawals

Read status and history through §35.12.

## 35.7 Conversions

POST /fx/quotes

POST /fx/conversions

Read status and history through §35.12.

## 35.8 Internal Transfers

GET /recipients/resolve?username=exampleuser

POST /transfers/internal

Confirm the resolved recipient per API §20.6. Both parties can retrieve their transfer through §35.12.

## 35.9 Pay Anyone

GET /banks?country=NG

POST /banks/resolve-account

POST /payments/bank/quotes

POST /payments/bank

Read payment history and individual receipts through §35.12.

## 35.10 External Wallet Transfers

External wallet sends use the withdrawal quote and execution routes in §35.6.

## 35.11 Beneficiaries

GET /beneficiaries

POST /beneficiaries

PATCH /beneficiaries/{beneficiary_id}

DELETE /beneficiaries/{beneficiary_id}

## 35.12 Transactions

GET /transactions

GET /transactions/{transaction_id}

## 35.13 Notifications

GET /notifications

POST /notifications/{notification_id}/read

## 35.14 Administration

Administrative endpoints must be separately authorized and must enforce role and permission checks.

The exact endpoint contracts, schemas, request bodies, response formats and OpenAPI specification will be finalized during backend architecture and implementation.

Every financial write endpoint must implement appropriate idempotency protection.

---

# 36. TRANSACTION STATE MACHINES

All financial transactions must have explicit state machines.

## 36.1 Internal Transfer

Validate and authorize the transfer, then commit the balanced posting and completion atomically. Use 05-LEDGER-ACCOUNTING.md §25.1 for internal states and 04-API-SPECIFICATION.md §38 for public status.

## 36.2 Deposit

Track detection, confirmations and screening before credit. Follow the exception and reorganization states in 05-LEDGER-ACCOUNTING.md §§25.2, 25.5.

## 36.3 External Withdrawal

Validate the quote and authorization, reserve the full debit, track submission, broadcast and confirmations, and complete on verified settlement. Use 05-LEDGER-ACCOUNTING.md §§25.3, 25.5.

## 36.4 Pay Anyone

Verify the beneficiary, quote and authorize the payment, reserve stablecoins, confirm conversion, then track the NGN payout. Complete on verified payout and posted settlement. Use 05-LEDGER-ACCOUNTING.md §§25.4–25.5, including for refunds.

## 36.5 Critical Failure Rule

A provider timeout does not mean a transaction failed.

If a bank payout times out after submission, DEYPAY must establish whether the payout was executed before releasing funds or retrying.

The system must prevent duplicate payouts.

Before conversion is confirmed, protect the stablecoin reservation. Once conversion is confirmed, consume it and carry the NGN payout obligation. An unknown payout leaves that obligation open. A confirmed failure enters the recovery process in 05-LEDGER-ACCOUNTING.md §19.8.

---

# 37. ERROR HANDLING AND FAILURE SCENARIOS

The backend must explicitly handle:

- Invalid requests.
- Duplicate requests.
- Invalid wallet addresses.
- Unsupported networks.
- Insufficient balance.
- Expired quotes.
- Provider authentication errors.
- Provider timeouts.
- Provider downtime.
- Failed payouts.
- Duplicate webhooks.
- Missing webhooks.
- Delayed blockchain confirmations.
- Blockchain reorganization where relevant.
- KYC provider downtime.
- Database errors.
- Queue processing failures.
- Reconciliation discrepancies.

Every error must be assigned an internal classification.

Customer-facing error messages must be understandable and must not reveal internal security details.

## 37.1 Retry Strategy

Retries must be limited and controlled.

Only operations that are safe to retry should be automatically retried.

Financial execution must use idempotency keys and provider references wherever supported.

Unknown outcomes must be reconciled before retrying.

## 37.2 Outbox and Event Processing

Financial database changes and the events required to process them must not become inconsistent.

The implementation should use a transactional outbox or equivalent reliable event publishing mechanism.

This prevents important events from being lost when an application process crashes after a database commit.

---

# 38. BUSINESS RULES

BR-001: DEYPAY is stablecoin-first.

BR-002: USDC and USDT are the initial customer wallet assets.

BR-003: Naira is a funding, conversion and settlement currency in the initial product model.

BR-004: Registration does not automatically authorize financial transactions.

BR-005: Required KYC and compliance approval must be obtained before financial activation.

BR-006: USDC and USDT wallet profiles should be automatically created for eligible customers.

BR-007: A username maps to a DEYPAY internal user ID, not a blockchain address.

BR-008: Internal transfers are recorded through DEYPAY's internal ledger, subject to regulatory approval.

BR-009: Pay Anyone recipients do not need DEYPAY accounts.

BR-010: Pay Anyone recipients receive NGN through an approved Nigerian bank payout arrangement.

BR-011: Pay Anyone defaults to NGN-first amount entry.

BR-012: All customer-facing conversion quotes must disclose relevant fees.

BR-013: Financial balances must be controlled by the internal ledger.

BR-014: Every posted journal must balance.

BR-015: Posted ledger entries must be immutable.

BR-016: Financial operations must support idempotency.

BR-017: Provider webhooks must be verified and deduplicated.

BR-018: Provider timeouts must not automatically trigger financial reversal.

BR-019: Every external financial operation must be traceable to an internal transaction.

BR-020: Administrative financial adjustments must be authorized and auditable.

BR-021: DEYPAY must maintain independent financial reconciliation.

BR-022: External provider integration must not replace DEYPAY's internal business logic.

BR-023: No financial activity may be enabled unless its regulatory and provider requirements are satisfied.

---

# 39. PERMISSIONS AND ACCESS CONTROL

The system shall support role-based access control.

Suggested roles:

1. CUSTOMER.
2. SUPPORT_AGENT.
3. KYC_REVIEWER.
4. COMPLIANCE_OFFICER.
5. FINANCE_OPERATOR.
6. OPERATIONS_ADMIN.
7. SYSTEM_ADMIN.
8. SUPER_ADMIN.

Permissions must be assigned by role and, where necessary, by individual action.

Sensitive financial actions must require elevated permissions.

Administrative access must be logged.

Super-admin access must not bypass financial audit controls.

---

# 40. MVP ACCEPTANCE CRITERIA

The MVP shall not be considered functionally complete until the following workflows are demonstrated successfully.

## 40.1 Registration

- A user can register.
- Email verification works.
- Phone verification works where required.
- Account is created.
- User can log in.
- User can explore non-financial areas.

## 40.2 KYC

- User can submit identity information.
- System can process verification results.
- Admin can review submissions.
- User receives approval or rejection status.
- Restricted financial actions remain unavailable before approval.

## 40.3 Wallet

- Eligible user receives USDC wallet profile.
- Eligible user receives USDT wallet profile.
- Wallet provisioning status is tracked.
- Supported addresses appear after provisioning.
- Wallets display correct assets and networks.

## 40.4 Deposit

- Incoming deposit is detected.
- Confirmation status is tracked.
- Deposit is screened.
- Correct customer is identified.
- Ledger is credited exactly once.
- Transaction history is updated.

## 40.5 Withdrawal

- Customer can enter a valid destination address.
- Correct network is selected.
- Quote and fees are displayed.
- Funds are reserved.
- Provider execution is tracked.
- Completed withdrawals are finalized.
- Failed withdrawals are handled safely.

## 40.6 Internal Transfers

- User can search for another DEYPAY user.
- Recipient is resolved correctly.
- Sender can authorize the transfer.
- Sender is debited.
- Recipient is credited.
- Both users are notified.
- Duplicate requests do not cause duplicate payments.

## 40.7 Pay Anyone

- User can select a Nigerian bank.
- User can enter an account number.
- Account name is resolved through the approved provider.
- User can enter an NGN amount.
- Stablecoin quote is retrieved.
- Fees are disclosed.
- Funds are reserved.
- Payout is executed through the approved partner.
- Final status is verified.
- Receipt is generated.

This acceptance criterion is conditional on the approved Nigerian payout, liquidity and regulatory arrangements being available.

## 40.8 Ledger

- All financial operations create balanced journals.
- Duplicate requests do not create duplicate postings.
- Posted entries cannot be edited.
- Reversals create compensating entries.
- Balances reconcile against ledger entries.

## 40.9 Administration

- Admin can search users.
- Admin can review KYC.
- Admin can investigate transactions.
- Admin can inspect provider references.
- Admin can review reconciliation.
- Admin permissions are enforced.
- Administrative actions are logged.

## 40.10 Reliability

- Provider timeouts do not cause duplicate payments.
- Duplicate webhooks are ignored safely.
- Missing webhooks can be recovered through reconciliation.
- Uncertain payouts remain in an appropriate state.
- Backend processing continues if the PWA is closed.

---

# 41. TESTING REQUIREMENTS

Testing must begin during backend development, not after the entire product is implemented.

## 41.1 Unit Testing

Every business module must have unit tests.

## 41.2 Integration Testing

Integration tests must cover:

- Authentication.
- Customer onboarding.
- KYC provider interactions.
- Wallet provisioning.
- Deposit processing.
- Withdrawal processing.
- FX quotes.
- Internal transfers.
- Pay Anyone.
- Provider webhooks.
- Ledger operations.
- Reconciliation.

## 41.3 Financial Testing

The system must test:

- Balanced journals.
- Negative balance prevention.
- Duplicate transactions.
- Concurrent transfers.
- Reservation release.
- Failed payouts.
- Reversals.
- Provider timeout recovery.
- Precision and rounding.
- Multi-asset accounting.

## 41.4 End-to-End Scenarios

At minimum, implement and test the following complete scenarios.

Scenario 1: Customer registers, completes KYC and receives USDC and USDT wallets.

Scenario 2: Customer receives an external USDC deposit and the correct ledger balance is credited.

Scenario 3: Customer sends USDT to an external wallet.

Scenario 4: Customer sends USDC to another DEYPAY user.

Scenario 5: Customer pays a Nigerian grocery store using USDT and the store receives NGN.

Scenario 6: Bank payout fails and the reserved customer balance is safely released after failure is confirmed.

Scenario 7: Bank payout times out after submission and DEYPAY successfully reconciles the outcome without duplicate payment.

Scenario 8: Duplicate provider webhooks are delivered and the ledger is credited only once.

Scenario 9: Two concurrent requests attempt to spend the same available balance and the system prevents overspending.

Scenario 10: A financial reconciliation discrepancy generates an operational alert.

## 41.5 Security Testing

Testing must include:

- Authentication attacks.
- Authorization bypass.
- Rate limiting.
- Injection vulnerabilities.
- Session security.
- Sensitive data exposure.
- Webhook forgery.
- Replay attacks.
- Insecure administrative actions.

## 41.6 Load Testing

The backend must be tested under expected concurrent request loads and increasing transaction volumes.

---

# 42. DEPLOYMENT AND INFRASTRUCTURE REQUIREMENTS

The initial infrastructure should include:

- Application hosting.
- PostgreSQL database.
- Redis where required.
- Durable asynchronous processing.
- Secure object storage.
- Secrets management.
- Logging.
- Monitoring.
- Error tracking.
- Backup infrastructure.
- Staging environment.
- Production environment.

## 42.1 Environments

Development:

Used by engineers during implementation.

Staging:

Used for integration testing and provider testing.

Production:

Used for approved live customer operations.

Production credentials must never be used in development.

## 42.2 Deployment

The application should support:

- Automated builds.
- Automated tests.
- Controlled deployment.
- Database migration management.
- Rollback procedures.
- Environment-specific configuration.

---

# 43. MONITORING AND OBSERVABILITY

The system must monitor:

- API response times.
- API errors.
- Database availability.
- Background jobs.
- Queue processing.
- Provider availability.
- Webhook processing.
- Transaction completion rates.
- Pending transactions.
- Failed payouts.
- Reconciliation discrepancies.
- Ledger inconsistencies.
- Authentication anomalies.

Financial alerts must be distinguishable from ordinary application errors.

Critical alerts must be delivered to the responsible operations personnel.

---

# 44. BACKUP AND DISASTER RECOVERY

DEYPAY must implement:

- Automated database backups.
- Backup encryption.
- Backup retention.
- Recovery testing.
- Point-in-time recovery where supported.
- Infrastructure recovery procedures.
- Provider outage procedures.
- Key management recovery procedures.
- Incident response procedures.

Initial recovery targets should be:

- Recovery Point Objective: 15 minutes or better for critical financial data, subject to infrastructure validation.
- Recovery Time Objective: 4 hours or better for critical backend services.

These are engineering targets that must be validated through actual recovery tests.

---

# 45. REGULATORY AND LEGAL DEPENDENCIES

DEYPAY's Nigerian operating model must be validated against applicable financial and virtual-asset laws and regulations.

The following activities require legal classification and appropriate permissions:

- Stablecoin custody.
- Stablecoin transfers.
- Customer wallet services.
- Platform-facilitated internal transfers.
- Stablecoin conversion.
- Naira settlement.
- Third-party bank payments.
- Payment gateway activity.
- Custodial financial services.
- Customer asset safeguarding.

The CBN's 2026 virtual-asset regulatory framework is a key dependency for the Nigerian operating model.

DEYPAY must not assume that an overseas provider's licences automatically authorize DEYPAY's activities in Nigeria.

## 45.1 Regulatory Responsibilities

Before production, the business must establish:

- DEYPAY's legal entity structure.
- Applicable Nigerian permissions.
- The roles of Nuvion and its regulated partners.
- Custody responsibilities.
- Asset safeguarding arrangements.
- Customer disclosures.
- AML/CFT responsibilities.
- Suspicious transaction reporting responsibilities.
- Data protection obligations.
- Consumer protection obligations.
- Complaints and dispute procedures.
- Transaction monitoring requirements.

## 45.2 Partner Agreements

All relevant provider agreements must clearly identify:

- Contracting legal entities.
- Services provided.
- Regulated entities involved.
- Customer eligibility.
- Custody responsibility.
- Settlement responsibility.
- Fees.
- Limits.
- Liability.
- Data processing.
- Service levels.
- Incident response.
- Termination.
- Asset return and migration procedures.

## 45.3 Legal Approval Gate

Production financial operations must not be enabled until the relevant legal and compliance review has approved the operating model.

The product owner reports affirmative verbal guidance on the general Pay Anyone capability. For live activation, retain the written provider and banking agreements, route-specific legal review, applicable operating approvals and evidence of the operational responsibilities in §25.7.

This is a release requirement, not a future enhancement.

---

# 46. MVP EXCLUSIONS

The following features are explicitly excluded from the initial MVP:

1. Gift card trading marketplace.
2. Prediction market trading.
3. Stock brokerage.
4. ETF trading.
5. Lending.
6. Borrowing.
7. Yield farming.
8. Staking products.
9. Savings products.
10. Copy trading.
11. Derivatives.
12. Advanced merchant acquiring.
13. Physical payment cards.
14. Complex business treasury services.

The architecture should allow future expansion without compromising the MVP's financial foundation.

---

# 47. FUTURE PRODUCT EXPANSION

## Phase 2: Gift Cards

Potential features:

- Buy gift cards.
- Sell gift cards.
- Gift card categories.
- Rate management.
- Submission and verification.
- Fraud controls.
- Settlement into supported stablecoin balances.
- Dispute management.

## Phase 3: Prediction Markets

Potential features:

- Market discovery.
- Market details.
- Yes/No positions.
- Position management.
- Settlement tracking.
- Market provider integration.

This requires separate legal and geographic eligibility assessment.

## Phase 4: Stocks and Investments

Potential features:

- Brokerage onboarding.
- Stock discovery.
- ETF discovery.
- Buy and sell orders.
- Portfolio tracking.
- Investment performance.
- Dividends.

This requires an appropriately regulated brokerage arrangement.

## Additional Expansion

- Virtual cards.
- Physical cards.
- Business accounts.
- Payroll.
- Merchant payments.
- International payments.
- Advanced multi-currency products.

---

# 48. IMPLEMENTATION ROADMAP

Development must follow a backend-first strategy.

## Phase 1: Product and Technical Foundation

Deliverables:

- Approved FRS.
- Technical architecture.
- Database design.
- API design.
- Financial ledger specification.
- Security design.
- Compliance requirements.
- Provider integration plan.

## Phase 2: Backend Core

Implement:

- Authentication.
- Customer management.
- KYC workflows.
- Wallet profiles.
- Financial ledger.
- Transaction engine.
- Idempotency.
- Audit logging.

## Phase 3: Provider Infrastructure

Implement:

- Provider abstraction.
- Nuvion API client.
- Wallet provisioning.
- Deposit integration.
- Withdrawal integration.
- FX integration.
- Webhook processing.
- Reconciliation.

Where production provider capabilities are not yet approved, use mock or sandbox adapters without enabling live financial activity.

## Phase 4: Payment Engine

Implement:

- Internal transfers.
- Beneficiary management.
- Bank account resolution.
- Pay Anyone.
- External wallet sends.
- Quote management.
- Payment state machines.

## Phase 5: Risk and Administration

Implement:

- Risk checks.
- Compliance workflows.
- Administrative dashboard APIs.
- Transaction investigations.
- Manual review.
- Maker-checker.
- Financial reporting.

## Phase 6: Backend Testing

Complete:

- Unit tests.
- Integration tests.
- Financial tests.
- End-to-end tests.
- Failure recovery tests.
- Security tests.
- Performance tests.

## Phase 7: PWA Development

Only after the backend foundations are stable:

- Design system.
- Registration screens.
- Login screens.
- KYC screens.
- Wallet dashboard.
- Deposit screens.
- Withdrawal screens.
- Conversion screens.
- Payments.
- Activity.
- Profile.
- Support.

## Phase 8: Controlled Launch

- Production provider approval.
- Legal and compliance approval.
- Financial reconciliation.
- Operational readiness.
- Security assessment.
- Pilot customers.
- Monitoring.
- Controlled rollout.

---

# 49. PROJECT DELIVERABLES

The project should produce the following artifacts.

## Product Deliverables

- Functional Requirements Specification.
- User journey documentation.
- Business rules.
- Acceptance criteria.
- Product roadmap.

## Engineering Deliverables

- System architecture document.
- Database schema.
- Entity relationship diagram.
- API specification.
- OpenAPI documentation.
- Rust backend source code.
- Provider integration adapters.
- Automated test suite.
- Deployment configuration.
- Monitoring configuration.

## Financial Deliverables

- Double-entry ledger specification.
- Chart of accounts.
- Accounting event mapping.
- Reconciliation procedures.
- Financial reporting definitions.
- Fee configuration rules.

## Compliance Deliverables

- Compliance responsibility matrix.
- KYC workflow.
- AML and sanctions procedures.
- Risk policy.
- Transaction limits.
- Customer terms and disclosures.
- Provider due-diligence records.
- Regulatory approval evidence.

## Operational Deliverables

- Admin dashboard.
- Customer support procedures.
- Incident response plan.
- Provider outage procedures.
- Disaster recovery procedures.
- Financial investigation procedures.

---

# 50. RISKS AND MITIGATION STRATEGIES

## Risk 1: Provider dependency

Risk: Nuvion becomes a single point of failure.

Mitigation: Implement provider-independent interfaces and reconciliation.

## Risk 2: Nigerian corridor limitations

Risk: Nuvion may not support every required NGN conversion or payout workflow.

Mitigation: Confirm corridors and maintain a separate Nigerian payout integration capability.

## Risk 3: Regulatory uncertainty

Risk: DEYPAY may not have the permissions required for certain financial activities.

Mitigation: Complete legal classification and obtain the necessary permissions before enabling those services.

## Risk 4: Custody risk

Risk: Customer assets may be inaccessible during provider failure.

Mitigation: Contractual safeguarding requirements, clear custody structure, reconciliation and recovery planning.

## Risk 5: Liquidity shortage

Risk: DEYPAY cannot settle customer bank payments.

Mitigation: Approved liquidity arrangements, prefunding, exposure limits and liquidity monitoring.

## Risk 6: Double spending

Risk: Concurrent transactions spend the same funds.

Mitigation: Atomic database operations, reservations and ledger constraints.

## Risk 7: Duplicate payouts

Risk: A provider retry results in multiple bank payments.

Mitigation: Idempotency, provider references and status reconciliation.

## Risk 8: Missing webhooks

Risk: A financial event is not delivered to DEYPAY.

Mitigation: Persistent event processing and scheduled provider reconciliation.

## Risk 9: Fraud

Risk: Accounts are used for fraudulent transactions.

Mitigation: KYC, transaction monitoring, screening, limits and investigations.

## Risk 10: Premature complexity

Risk: The team builds too many products before the wallet foundation is stable.

Mitigation: Keep MVP scope focused and stage future products.

---

# 51. OUTSTANDING DECISIONS AND DEPENDENCIES

The following items must be resolved before the relevant production features are enabled.

## 51.1 Nuvion

- Direct Nigerian business onboarding.
- Nigerian customer eligibility.
- Production USDC and USDT support.
- Supported production networks.
- USDC/NGN conversion.
- USDT/NGN conversion.
- Nigerian bank payout support.
- Bank account name resolution.
- Third-party bank payout capability.
- Custody arrangements.
- Liquidity model.
- Settlement timelines.
- Commercial pricing.
- Transaction limits.
- Production SLA.
- Legal entities and regulated partners.
- Production credentials.

## 51.2 Other Financial Providers

- Nigerian bank payout backup provider.
- KYC provider.
- AML and sanctions screening provider.
- Blockchain monitoring provider.
- Messaging provider.

## 51.3 Product Decisions

- Initial supported blockchain networks.
- Final fee schedule.
- Initial transaction limits.
- Username change policy.
- Phone number discoverability policy.
- Payment link policy.
- Initial supported customer jurisdictions.
- Initial supported bank list.

## 51.4 Legal and Compliance

- DEYPAY's operating entity.
- Applicable licensing and authorization route.
- Custody responsibility.
- Stablecoin activity permissions.
- Payment activity permissions.
- AML/CFT responsibility.
- Customer terms and disclosures.
- Data protection and retention requirements.

These are explicit dependencies. They must not be silently treated as confirmed capabilities.

---

# 52. DEFINITION OF DONE

A feature is not complete merely because its user interface works.

A financial feature is complete only when:

1. The backend workflow is implemented.
2. Business rules are enforced.
3. The financial ledger is correctly updated.
4. The transaction state machine works.
5. Idempotency is implemented.
6. Failure cases are handled.
7. Authorization is enforced.
8. Audit logs are generated.
9. Automated tests pass.
10. Provider integration works or an explicitly documented simulation is used.
11. Reconciliation is supported.
12. API documentation is updated.
13. Security requirements are satisfied.
14. Acceptance criteria pass.

For production financial features, all required regulatory and partner approvals must also be completed.

---

# 53. FINAL PRODUCT DEFINITION

DEYPAY is an independent stablecoin-first financial platform that allows eligible users to hold, receive, send, convert and spend USDC and USDT through a simple customer-facing application.

Its core capabilities are:

1. A secure customer account.
2. USDC and USDT wallets.
3. Stablecoin deposits.
4. Stablecoin withdrawals.
5. NGN on-ramp.
6. NGN off-ramp.
7. Currency conversion.
8. Internal DEYPAY transfers.
9. Stablecoin-funded Nigerian bank payments.
10. External blockchain transfers.
11. Transaction tracking.
12. Customer support.
13. Risk and compliance.
14. Financial accounting.
15. Administrative operations.

DEYPAY will own its product, backend, customer database, financial ledger, payment logic, transaction history, pricing presentation and operational environment.

Nuvion will be pursued as a preferred infrastructure partner for the financial services it can contractually, technically and legally provide.

Other providers may be integrated where necessary.

The backend will be developed in Rust using a modular monolith architecture, with PostgreSQL as the primary relational database and clearly defined provider-independent interfaces.

The initial customer application will be a responsive web application and Progressive Web App.

The development strategy is backend-first: financial foundations, ledger, wallet, transactions, payments, provider integrations, compliance and automated testing must be implemented before the frontend becomes the main development focus.

The long-term product vision is to expand DEYPAY into a broader financial ecosystem that includes gift cards, prediction markets, stocks and additional payment services.

**FINAL PRODUCT PRINCIPLE**

DEYPAY owns the product and controls the customer experience.

DEYPAY owns its financial records and business logic.

Infrastructure partners provide the external services required to move and safeguard money.

Every financial operation must be traceable, reconciled, authorized and auditable.

The customer should experience one unified product: DEYPAY.

---

**END OF DEYPAY FUNCTIONAL REQUIREMENTS SPECIFICATION – VERSION 1.0**