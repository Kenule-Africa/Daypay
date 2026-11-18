# DEYPAY (DP)
# SECURITY ARCHITECTURE AND THREAT MODEL
## Identity Security, Financial Transaction Protection, Custodial Wallet Controls, Infrastructure Hardening, Fraud Prevention and Incident Response

**Document ID:** DP-SEC-006  
**Version:** 1.0  
**Status:** Proposed Engineering Baseline — Pending Security, Legal and Provider Validation  
**Product:** DEYPAY  
**Organization:** Kenule Africa  
**Initial Market:** Nigeria  
**Backend:** Rust / Axum / Tokio / SQLx  
**Database:** PostgreSQL  
**Architecture:** Modular Monolith  
**Frontend:** Responsive Progressive Web Application (PWA)  
**Future Platform:** Native Mobile Applications  
**Initial Customer Assets:** USDC and USDT  
**Fiat Settlement Currency:** NGN  
**Preferred Infrastructure Candidate:** Nuvion  
**Date:** 18 November 2026  
**Classification:** Confidential — Internal Security and Engineering

**Related Documents**

- DP-001 — Functional Requirements Specification
- DP-002 — System Architecture Document
- DP-003 — Database Design Specification
- DP-005 — Financial Ledger and Accounting Design Specification
- DP-004 — API Specification and Contract Design
- DP-007 — Software Test Strategy and Quality Assurance Specification (07-TEST-STRATEGY.md)
- DP-008 — Software Implementation Plan (08-IMPLEMENTATION-PLAN.md)

---

# DOCUMENT CONTROL

## 1. Purpose

This document defines the proposed security architecture, threat model, security requirements and operational safeguards for DEYPAY.

It establishes how the platform must protect:

- Customer identities.
- Authentication credentials.
- Active sessions.
- Personal and KYC information.
- USDC and USDT balances.
- Custodial wallet operations.
- Financial transactions.
- Nigerian bank payments.
- Financial ledger records.
- Provider credentials.
- Administrative access.
- Treasury operations.
- Infrastructure.
- Audit records.
- Recovery processes.

This specification provides a security baseline for implementation, testing and production readiness.

It does not certify that DEYPAY is secure, compliant or authorized to operate. Those conclusions require implementation evidence, independent assessment and applicable regulatory approval.

## 2. Document Ownership

**Business Owner:** Kenule Africa / DEYPAY Product Leadership

**Technical Owner:** DEYPAY Engineering Lead

**Security Owner:** Designated Security Lead

**Reviewers:** Compliance, Financial Operations, Infrastructure, Legal and Independent Security Assessors

## 3. Approval Requirements

Production approval requires review of:

1. Security architecture.
2. Threat model.
3. Authentication and authorization controls.
4. Custody responsibilities.
5. Financial transaction security.
6. Provider integration security.
7. Data protection.
8. Infrastructure security.
9. Incident response readiness.
10. Security testing evidence.

---

# TABLE OF CONTENTS

1. Executive Summary
2. Security Objectives
3. Security Principles
4. Security Scope
5. System Security Architecture
6. Trust Boundaries
7. Critical Assets
8. Threat Actors
9. Threat Modeling Methodology
10. Threat Register
11. Identity and Authentication Security
12. Session Management
13. Authorization and Access Control
14. Customer Account Recovery
15. Financial Transaction Authorization
16. Custodial Wallet and Private Key Security
17. Stablecoin Deposit Security
18. External Withdrawal Security
19. Internal Transfer Security
20. NGN Bank Payment Security
21. FX Conversion Security
22. Financial Ledger Security
23. Provider Integration Security
24. Webhook Security
25. API Security
26. PWA and Browser Security
27. Mobile Application Security
28. Data Protection and Encryption
29. KYC and Sensitive Identity Data
30. Database Security
31. Infrastructure and Network Security
32. Secrets Management
33. Software Supply Chain Security
34. Secure Rust Development
35. Administrative Security
36. Treasury and Financial Operations Security
37. Fraud Prevention and Transaction Monitoring
38. Logging, Monitoring and Detection
39. Incident Response
40. Business Continuity and Disaster Recovery
41. Security Testing and Assurance
42. Vulnerability Management
43. Third-Party Security Governance
44. Regulatory and Compliance Dependencies
45. Security Non-Functional Requirements
46. Security Implementation Roadmap
47. Security Acceptance Criteria
48. Open Decisions and Dependencies
49. Final Security Architecture Definition
50. Approval and Sign-Off

---

# 1. EXECUTIVE SUMMARY

DEYPAY will implement a defense-in-depth security architecture.

Security controls must protect the platform across multiple independent layers:

1. Customer identity.
2. Authentication.
3. Authorization.
4. Financial transaction approval.
5. Business logic.
6. Financial ledger.
7. External provider execution.
8. Data storage.
9. Infrastructure.
10. Monitoring and incident response.

DEYPAY's financial operating model introduces risks that do not exist in ordinary non-financial applications.

These include:

- Customer account takeover.
- Unauthorized stablecoin withdrawals.
- Fraudulent Nigerian bank payouts.
- Duplicate payment execution.
- Custodial infrastructure compromise.
- Provider API credential theft.
- Incorrect or manipulated FX quotes.
- Ledger tampering.
- Treasury fund diversion.
- Insider abuse.
- Identity theft.
- Sensitive data exposure.
- Provider compromise.
- Operational outages.

The security architecture must ensure that compromising one component does not automatically grant control over customer funds.

DEYPAY will retain control of its own application security, customer authorization, business rules and financial records.

Nuvion and other providers will be responsible only for the security obligations assigned to them under approved technical and contractual arrangements.

**Security must be treated as a continuous engineering and operational responsibility.**

---

# 2. SECURITY OBJECTIVES

## SEC-OBJ-001 — Confidentiality

Protect sensitive customer, financial and operational information against unauthorized disclosure.

## SEC-OBJ-002 — Integrity

Prevent unauthorized or undetected modification of financial and application data.

## SEC-OBJ-003 — Availability

Maintain reliable access to critical services and recover safely from failures.

## SEC-OBJ-004 — Financial Authorization

Ensure that financial transactions are executed only after appropriate customer or authorized operational approval.

## SEC-OBJ-005 — Custody Protection

Protect customer assets through an approved custodial security model.

## SEC-OBJ-006 — Fraud Resistance

Detect, prevent and investigate suspicious activity.

## SEC-OBJ-007 — Accountability

Maintain traceable records of sensitive actions.

## SEC-OBJ-008 — Recoverability

Support safe recovery after incidents without creating duplicate or unauthorized financial movements.

## SEC-OBJ-009 — Provider Independence

Prevent external infrastructure integrations from bypassing DEYPAY's internal security controls.

## SEC-OBJ-010 — Compliance Readiness

Produce the controls and evidence needed for applicable security, privacy and financial regulatory reviews.

---

# 3. SECURITY PRINCIPLES

## 3.1 Zero Trust

No request, device, service or administrator is trusted solely because it is connected to an internal network.

## 3.2 Least Privilege

Every identity receives only the permissions necessary for its responsibilities.

## 3.3 Defense in Depth

Multiple independent safeguards must protect critical assets.

## 3.4 Secure by Default

New features must begin in the safest practical configuration.

## 3.5 Deny by Default

Access must be denied unless explicitly authorized.

## 3.6 Separation of Duties

No single administrator should be able to initiate, approve and conceal high-risk financial operations.

## 3.7 Financial Integrity First

Security controls must protect the ledger and external settlement workflows even when this increases processing time.

## 3.8 Assume Compromise

The architecture must support containment, credential rotation, investigation and recovery after a component is compromised.

## 3.9 Minimize Sensitive Data

DEYPAY should collect and retain only information necessary for legitimate product, operational and regulatory purposes.

## 3.10 Security Evidence

Controls must be verifiable through tests, logs, configuration reviews and independent assessments.

---

# 4. SECURITY SCOPE

## 4.1 Included Systems

- Customer PWA.
- Future mobile applications.
- Rust API backend.
- Authentication services.
- Customer profile services.
- KYC orchestration.
- Financial ledger.
- Wallet services.
- Payment orchestration.
- Background workers.
- Provider adapters.
- PostgreSQL.
- Object storage.
- Administrative dashboard.
- Treasury interfaces.
- Logging and monitoring.
- CI/CD pipelines.
- Cloud infrastructure.
- Backup and recovery systems.

## 4.2 External Dependencies

Potential external dependencies include:

- Nuvion.
- Custody infrastructure.
- KYC providers.
- Nigerian banking partners.
- Fiat conversion providers.
- Blockchain networks.
- Email and SMS providers.
- Cloud hosting providers.

## 4.3 Scope Limitation

The internal security architecture cannot independently guarantee the security of external providers.

Provider controls must be assessed and contractually established.

---

# 5. SYSTEM SECURITY ARCHITECTURE

## 5.1 High-Level Security Architecture

```text
CUSTOMER PWA / MOBILE
          |
          | HTTPS / TLS
          v
EDGE SECURITY LAYER
WAF / RATE LIMITING / DDoS CONTROLS
          |
          v
DEYPAY RUST API
          |
          +-------------------------+
          |                         |
          v                         v
AUTHENTICATION               AUTHORIZATION
SESSION SECURITY             POLICY ENGINE
          |                         |
          +------------+------------+
                       |
                       v
              APPLICATION SERVICES
                       |
          +------------+------------+
          |                         |
          v                         v
FINANCIAL LEDGER          PAYMENT ORCHESTRATION
          |                         |
          v                         v
     POSTGRESQL              PROVIDER ADAPTERS
                                    |
                                    v
                          NUVION / OTHER PROVIDERS

SEPARATE CONTROL PLANE:
ADMIN ACCESS / TREASURY APPROVALS
AUDIT LOGGING / MONITORING / INCIDENT RESPONSE
```

## 5.2 Security Enforcement

Security controls must exist at the appropriate layers.

For example:

- The edge limits abusive traffic.
- Authentication establishes identity.
- Authorization determines permitted actions.
- Business logic enforces financial rules.
- The ledger prevents invalid postings.
- Provider adapters restrict external execution.
- Monitoring detects suspicious outcomes.

## 5.3 Critical Rule

No frontend request may directly cause a provider payout without passing through DEYPAY's approved authorization, risk, reservation and orchestration controls.

---

# 6. TRUST BOUNDARIES

## 6.1 Boundary TB-001 — Customer Device to DEYPAY

Customer devices are untrusted.

Requests must be authenticated, validated and authorized.

## 6.2 Boundary TB-002 — Public Internet to API

Internet traffic must pass through approved edge controls.

## 6.3 Boundary TB-003 — API to Financial Ledger

Only authorized application services may initiate financial postings.

## 6.4 Boundary TB-004 — DEYPAY to External Providers

Provider requests must use authenticated, encrypted communication.

## 6.5 Boundary TB-005 — External Provider to Webhook Receiver

Incoming provider events must be independently authenticated and verified.

## 6.6 Boundary TB-006 — Administration to Production

Administrative access must use strong authentication and controlled permissions.

## 6.7 Boundary TB-007 — Development to Production

Development credentials and personnel must not automatically possess production access.

## 6.8 Boundary TB-008 — Backup and Recovery

Backups must remain protected independently of primary production systems.

---

# 7. CRITICAL ASSETS

## 7.1 Asset Classification

| Asset | Classification | Primary Security Concern |
|---|---|---|
| Customer credentials | Restricted | Account takeover |
| Authentication sessions | Restricted | Session hijacking |
| KYC documents | Highly restricted | Identity theft |
| Customer personal data | Confidential | Privacy breach |
| Ledger journals | Critical financial | Tampering |
| Customer balances | Critical financial | Unauthorized value movement |
| Provider API credentials | Critical secret | External fund movement |
| Custody signing authority | Critical secret | Asset theft |
| Treasury credentials | Critical secret | Fund diversion |
| Administrative credentials | Highly restricted | Privilege abuse |
| Reconciliation records | Critical financial | Concealed discrepancies |
| Audit logs | Security critical | Evidence destruction |
| Source code | Confidential | Vulnerability exposure |
| Database backups | Highly restricted | Bulk data compromise |

## 7.2 Asset Ownership

Every critical asset must have an assigned operational owner.

## 7.3 Asset Inventory

The organization must maintain an inventory of systems, data stores, secrets, integrations and privileged identities.

---

# 8. THREAT ACTORS

## 8.1 External Cybercriminals

Attackers seeking account access, funds or sensitive information.

## 8.2 Fraudulent Customers

Users attempting unauthorized transfers, identity fraud, duplicate credits or payment abuse.

## 8.3 Compromised Customer Devices

Devices affected by malware, phishing or credential theft.

## 8.4 Malicious Insiders

Personnel abusing legitimate access.

## 8.5 Compromised Providers

External providers whose systems or credentials have been breached.

## 8.6 Automated Attackers

Bots attempting credential stuffing, enumeration or service disruption.

## 8.7 Supply Chain Attackers

Attackers compromising dependencies, build pipelines or software distribution.

## 8.8 Social Engineering Attackers

Individuals manipulating customers or staff into authorizing unsafe actions.

---

# 9. THREAT MODELING METHODOLOGY

## 9.1 Method

DEYPAY will use a structured threat modeling approach informed by STRIDE.

STRIDE categories:

- Spoofing.
- Tampering.
- Repudiation.
- Information disclosure.
- Denial of service.
- Elevation of privilege.

## 9.2 Risk Assessment

Threats must be assessed by:

- Likelihood.
- Financial impact.
- Customer impact.
- Operational impact.
- Regulatory impact.
- Detectability.

## 9.3 Risk Levels

- Critical.
- High.
- Medium.
- Low.

## 9.4 Review Triggers

The threat model must be reviewed when:

- A new provider is introduced.
- A new blockchain network is supported.
- A new financial product is launched.
- Custody arrangements change.
- Administrative permissions change.
- A material security incident occurs.
- The architecture changes significantly.

---

# 10. THREAT REGISTER

| ID | Threat | Risk | Required Mitigation |
|---|---|---|---|
| TH-001 | Customer account takeover | Critical | MFA, session controls, risk monitoring |
| TH-002 | Unauthorized stablecoin withdrawal | Critical | Step-up authorization, destination checks |
| TH-003 | Duplicate bank payout | Critical | Idempotency, provider reconciliation |
| TH-004 | Ledger tampering | Critical | Restricted posting, immutable journals |
| TH-005 | Provider API credential theft | Critical | Secret isolation, rotation, least privilege |
| TH-006 | Custody signing compromise | Critical | Custodian controls, transaction policies |
| TH-007 | Administrator privilege abuse | Critical | RBAC, maker-checker, audit |
| TH-008 | KYC document exposure | High | Encryption, restricted access |
| TH-009 | Session hijacking | High | Secure sessions, revocation |
| TH-010 | Webhook forgery | High | Signature verification, event validation |
| TH-011 | FX quote manipulation | High | Server-side quote binding |
| TH-012 | Bank beneficiary substitution | Critical | Verified beneficiary binding |
| TH-013 | API enumeration | Medium | Rate limiting, privacy controls |
| TH-014 | Dependency compromise | High | Supply chain controls |
| TH-015 | Database exfiltration | Critical | Network isolation, encryption |
| TH-016 | Backup compromise | High | Encrypted isolated backups |
| TH-017 | Provider outage | High | Safe state handling, reconciliation |
| TH-018 | Payment workflow race condition | Critical | Atomic reservations, locking |
| TH-019 | Social engineering of support | High | Identity verification, restricted actions |
| TH-020 | Audit log destruction | High | Tamper-resistant external logging |

Each threat must be assigned an owner and tracked to implementation and test evidence.

---

# 11. IDENTITY AND AUTHENTICATION SECURITY

## SEC-AUTH-001 — Unique Identity

Every customer must have an immutable internal identifier.

## SEC-AUTH-002 — Password Storage

Passwords must be hashed using Argon2id or another approved password hashing algorithm.

Plaintext passwords must never be stored.

## SEC-AUTH-003 — Password Policy

Passwords must satisfy an approved policy emphasizing resistance to guessing and known compromised credentials.

## SEC-AUTH-004 — Login Protection

Login endpoints must implement:

- Rate limiting.
- Abuse detection.
- Generic failure responses.
- Suspicious activity monitoring.

## SEC-AUTH-005 — Multifactor Authentication

MFA must be supported.

Phishing-resistant methods, including passkeys where feasible, are preferred for high-risk access.

## SEC-AUTH-006 — Authentication Events

Security-relevant events must be recorded.

## SEC-AUTH-007 — Email and Phone Verification

Verification codes must expire and be rate-limited.

## SEC-AUTH-008 — Account Enumeration

Authentication responses must avoid unnecessarily revealing whether a customer account exists.

---

# 12. SESSION MANAGEMENT

## SEC-SES-001 — Session Tokens

Session tokens must be cryptographically unpredictable.

## SEC-SES-002 — Token Expiration

Use the server-side opaque PWA session in 04-API-SPECIFICATION.md §6. Enforce idle and absolute expiry server-side; authenticated refresh can renew idle time only within the absolute lifetime.

## SEC-SES-003 — Secure Cookies

For browser sessions, use Secure, HttpOnly and appropriate SameSite attributes.

## SEC-SES-004 — CSRF Protection

Cookie-authenticated state-changing requests must be protected against cross-site request forgery.

## SEC-SES-005 — Session Rotation

Rotate the session identifier on login, refresh and privilege changes. Invalidate the previous identifier atomically.

## SEC-SES-006 — Session Revocation

Users must be able to revoke active sessions.

## SEC-SES-007 — Suspicious Sessions

High-risk sessions may require additional verification or termination.

## SEC-SES-008 — Token Storage

Long-lived authentication secrets must not be stored in browser localStorage.

---

# 13. AUTHORIZATION AND ACCESS CONTROL

## SEC-AZ-001 — Deny by Default

Unauthorized requests must be rejected.

## SEC-AZ-002 — Resource Ownership

Customers must access only resources they own or are permitted to access.

## SEC-AZ-003 — Role-Based Access Control

Administrative permissions must use defined roles.

## SEC-AZ-004 — Policy-Based Authorization

Sensitive operations must also consider context such as:

- KYC status.
- Account restrictions.
- Transaction amount.
- Device risk.
- Beneficiary risk.
- Transaction limits.

## SEC-AZ-005 — Server Enforcement

Frontend controls must never replace backend authorization.

## SEC-AZ-006 — Privilege Changes

Administrative privilege changes must be logged and approved.

---

# 14. CUSTOMER ACCOUNT RECOVERY

## SEC-REC-001 — Recovery Security

Account recovery must not provide an easier path to financial access than ordinary authentication.

## SEC-REC-002 — Identity Verification

Recovery must use an approved identity verification process.

## SEC-REC-003 — Recovery Risk

High-risk recovery attempts must trigger enhanced review.

## SEC-REC-004 — Session Revocation

Successful credential or MFA recovery must revoke all sessions and outstanding transaction challenges and proofs. Require fresh login with the recovered credentials and record the recovery in the audit log.

## SEC-REC-005 — Withdrawal Restrictions

After sensitive recovery, hold new withdrawals and bank payments until the approved cooling-off period elapses and fresh step-up verification succeeds. Route unresolved risk to authorized review. Risk and Security own the duration and release criteria.

## SEC-REC-006 — Support Restrictions

Support agents must not bypass recovery controls.

---

# 15. FINANCIAL TRANSACTION AUTHORIZATION

## SEC-TXN-001 — Explicit Authorization

Every customer-initiated financial operation must have an attributable authorization event.

## SEC-TXN-002 — Transaction Binding

Authorization must be bound to:

- Customer.
- Transaction type.
- Asset.
- Amount.
- Immutable recipient ID or verified destination details (bank and account, or chain address and memo/tag).
- Network for blockchain operations.
- Applicable fees.
- Quote where relevant.
- Expiration.

## SEC-TXN-003 — Authorization Expiry

Authorization proofs must be short-lived.

## SEC-TXN-004 — Single Use

Each issued transaction proof must be single-use and tied to the authenticated customer and session. Consume it in the transaction accepting the financial operation. Replay an already accepted identical request from its idempotency record.

## SEC-TXN-005 — Transaction Changes

Any material change to the transaction must invalidate the previous authorization.

## SEC-TXN-006 — Risk-Based Step-Up

High-risk transactions must require additional verification.

## SEC-TXN-007 — Backend Enforcement

The backend must validate authorization before reserving or executing funds.

---

# 16. CUSTODIAL WALLET AND PRIVATE KEY SECURITY

## 16.1 Custody Model

DEYPAY's proposed MVP uses external custodial infrastructure.

The exact custody model must be confirmed before production.

## SEC-CUS-001 — Custody Responsibility

The legal and technical custodian must be identified.

## SEC-CUS-002 — Key Ownership

The parties controlling private keys or signing authority must be documented.

## SEC-CUS-003 — No Frontend Keys

Private keys must never be embedded in frontend applications.

## SEC-CUS-004 — No Plaintext Key Storage

If DEYPAY ever controls signing keys, they must not be stored as plaintext in application databases or environment files.

## SEC-CUS-005 — Custodian Assessment

The custodian must be assessed for:

- Key generation.
- Key storage.
- Signing policies.
- Access controls.
- Recovery.
- Incident response.
- Asset segregation.
- Audit evidence.

## SEC-CUS-006 — Signing Policies

Where supported, custody operations must enforce:

- Transaction limits.
- Destination restrictions.
- Approval thresholds.
- Risk screening.
- Emergency suspension.

## SEC-CUS-007 — Emergency Controls

DEYPAY must be able to suspend new withdrawal requests.

The ability to stop provider-side signing or settlement must be established contractually and technically.

## SEC-CUS-008 — Custody Reconciliation

Customer liabilities must be reconciled against underlying custody evidence.

## SEC-CUS-009 — Key Recovery

Any recovery or migration process must be documented and tested.

## SEC-CUS-010 — Provider Exit

DEYPAY must maintain an approved plan for customer asset and data continuity if the custody provider changes.

---

# 17. STABLECOIN DEPOSIT SECURITY

## SEC-DEP-001 — Supported Networks

Only approved asset and network combinations may be presented.

## SEC-DEP-002 — Address Ownership

Deposit addresses must be mapped to verified internal customer records.

## SEC-DEP-003 — Blockchain Confirmation

Deposits must satisfy configured finality requirements.

## SEC-DEP-004 — Duplicate Prevention

The same verified deposit must not be credited twice.

## SEC-DEP-005 — Chain Reorganizations

The system must detect and respond to relevant blockchain reorganizations.

## SEC-DEP-006 — Unsupported Tokens

Unsupported token deposits must not automatically create customer balances.

## SEC-DEP-007 — Deposit Screening

Deposits must be subject to applicable compliance and risk controls.

## SEC-DEP-008 — Provider Verification

Provider deposit events must be validated before ledger credit.

---

# 18. EXTERNAL WITHDRAWAL SECURITY

## SEC-WDR-001 — Destination Validation

Withdrawal addresses must be validated for the selected network.

## SEC-WDR-002 — Network Confirmation

The customer must confirm the selected network.

## SEC-WDR-003 — Step-Up Authentication

Sensitive withdrawals must require additional authorization.

## SEC-WDR-004 — Available Balance

The backend must verify available funds.

## SEC-WDR-005 — Reservation

Funds must be reserved before external execution.

## SEC-WDR-006 — Idempotency

Repeated requests must not create duplicate withdrawals.

## SEC-WDR-007 — Provider Execution

External execution must use authenticated provider communication.

## SEC-WDR-008 — Unknown Outcomes

Timeouts must not trigger blind re-execution.

## SEC-WDR-009 — Settlement Verification

Completion must be based on approved settlement evidence.

## SEC-WDR-010 — Withdrawal Monitoring

Suspicious withdrawals must be detected and escalated.

---

# 19. INTERNAL TRANSFER SECURITY

## SEC-INT-001 — Recipient Verification

Recipient identity must be resolved through trusted backend records.

## SEC-INT-002 — Ownership

The sender must be authenticated and eligible.

## SEC-INT-003 — Balance Validation

Available balance must be sufficient.

## SEC-INT-004 — Atomicity

Sender debit and recipient credit must occur atomically.

## SEC-INT-005 — Concurrency Protection

Concurrent transfers must not overspend the same balance.

## SEC-INT-006 — Fraud Monitoring

Unusual transfer patterns must be monitored.

## SEC-INT-007 — Transfer Limits

Applicable transaction limits must be enforced.

---

# 20. NGN BANK PAYMENT SECURITY

## SEC-BNK-001 — Approved Banking Infrastructure

Bank payouts must use approved providers.

## SEC-BNK-002 — Beneficiary Verification

Bank account details must be verified through trusted infrastructure.

## SEC-BNK-003 — Beneficiary Binding

Customer authorization must bind to the exact bank and account details.

## SEC-BNK-004 — Name Confirmation

The recipient's verified account name must be displayed before payment authorization.

## SEC-BNK-005 — Quote Integrity

The payment quote must be generated and validated by the backend.

## SEC-BNK-006 — Funds Reservation

Customer stablecoins must be reserved before external execution.

## SEC-BNK-007 — Duplicate Prevention

The same payment must not execute twice.

## SEC-BNK-008 — Payout Confirmation

Bank payment completion must be verified.

## SEC-BNK-009 — Failed Payouts

Failed and uncertain payouts must follow controlled recovery procedures.

## SEC-BNK-010 — Regulatory Gate

Third-party bank payouts must remain disabled until the applicable legal and provider requirements are satisfied.

---

# 21. FX CONVERSION SECURITY

## SEC-FX-001 — Server-Side Quotes

Executable quotes must originate from trusted backend processes.

## SEC-FX-002 — Quote Expiration

Expired quotes must be rejected.

## SEC-FX-003 — Quote Binding

Quotes must be bound to the correct customer and transaction parameters.

## SEC-FX-004 — Fee Integrity

Fees and conversion amounts must not be modified by the frontend.

## SEC-FX-005 — Provider Validation

Provider execution must match the accepted quote or follow an explicitly approved deviation policy.

## SEC-FX-006 — Settlement Verification

Conversion completion must be verified before final accounting recognition.

---

# 22. FINANCIAL LEDGER SECURITY

## SEC-LED-001 — Restricted Posting

Only approved services may post financial journals.

## SEC-LED-002 — Double-Entry Enforcement

Every posted journal must balance independently by asset.

## SEC-LED-003 — Immutability

Posted journals must not be edited or deleted.

## SEC-LED-004 — Idempotency

Duplicate accounting events must not create duplicate postings.

## SEC-LED-005 — Atomic Transactions

All entries in a journal must commit atomically.

## SEC-LED-006 — Balance Protection

Reservations and transfers must prevent double spending.

## SEC-LED-007 — Administrative Controls

Administrators must not directly edit balances.

## SEC-LED-008 — Reconciliation

Ledger balances must be reconciled with external financial evidence.

## SEC-LED-009 — Auditability

Every financial posting must be traceable to its originating event.

---

# 23. PROVIDER INTEGRATION SECURITY

## SEC-PRV-001 — Credential Isolation

Provider credentials must be stored in approved secrets management systems.

## SEC-PRV-002 — Least Privilege

Provider credentials must have the minimum required permissions.

## SEC-PRV-003 — Environment Separation

Development and production provider credentials must be separate.

## SEC-PRV-004 — Secure Transport

Provider communication must use TLS.

## SEC-PRV-005 — Request Validation

Provider responses must be treated as untrusted external data.

## SEC-PRV-006 — Provider Status Verification

Financial state changes must be based on validated provider evidence.

## SEC-PRV-007 — Credential Rotation

Credentials must support controlled rotation.

## SEC-PRV-008 — Provider Monitoring

Provider failures, timeouts and unusual responses must be monitored.

## SEC-PRV-009 — Provider Independence

Provider integrations must not bypass DEYPAY's internal authorization and ledger controls.

## SEC-PRV-010 — Contractual Controls

Provider contracts must establish security responsibilities, incident notification requirements and operational support.

---

# 24. WEBHOOK SECURITY

## SEC-WHK-001 — Authentication

Provider webhooks must be authenticated using the provider's supported verification method.

## SEC-WHK-002 — Signature Validation

Where signatures are supported, they must be validated against the original request body.

## SEC-WHK-003 — Replay Protection

Replay attacks must be mitigated through event uniqueness and applicable timestamp or nonce checks.

## SEC-WHK-004 — Deduplication

Duplicate webhook events must not cause duplicate financial postings.

## SEC-WHK-005 — Event Validation

Webhook events must be matched against expected provider operations.

## SEC-WHK-006 — Safe Acknowledgment

Events should be durably recorded before successful acknowledgment.

## SEC-WHK-007 — Out-of-Order Handling

Out-of-order events must not cause invalid state transitions.

## SEC-WHK-008 — Monitoring

Webhook verification failures must be monitored.

---

# 25. API SECURITY

## SEC-API-001 — HTTPS

Production APIs must use HTTPS.

## SEC-API-002 — Authentication

Protected endpoints must require valid authentication.

## SEC-API-003 — Authorization

Object-level authorization must be enforced.

## SEC-API-004 — Input Validation

All request payloads must be validated.

## SEC-API-005 — Request Limits

Request body sizes and processing limits must be enforced.

## SEC-API-006 — Rate Limiting

Sensitive endpoints must be rate-limited.

## SEC-API-007 — Idempotency

Financial mutation endpoints must support durable idempotency.

## SEC-API-008 — Error Safety

Error responses must not expose secrets or internal stack traces.

## SEC-API-009 — Security Headers

Appropriate HTTP security headers must be configured.

## SEC-API-010 — API Documentation

Security requirements must be represented in OpenAPI contracts.

---

# 26. PWA AND BROWSER SECURITY

## SEC-WEB-001 — Content Security Policy

A restrictive Content Security Policy must be configured.

## SEC-WEB-002 — XSS Protection

Untrusted content must be safely rendered.

## SEC-WEB-003 — CSRF Protection

Cookie-authenticated mutation requests must use CSRF protection.

## SEC-WEB-004 — Secure Cookies

Session cookies must use secure attributes.

## SEC-WEB-005 — Sensitive Storage

Authentication secrets must not be stored in browser localStorage.

## SEC-WEB-006 — Dependency Security

Frontend dependencies must be monitored for vulnerabilities.

## SEC-WEB-007 — Service Worker Security

Service workers must not cache sensitive authenticated financial responses.

## SEC-WEB-008 — Offline Behavior

Offline mode must not simulate completed financial transactions.

## SEC-WEB-009 — Third-Party Scripts

Third-party scripts must be minimized and reviewed.

## SEC-WEB-010 — Transaction Confirmation

The frontend must clearly display the final recipient, amount, asset and fees before authorization.

---

# 27. MOBILE APPLICATION SECURITY

Future native mobile applications must implement:

- Secure credential storage.
- Platform-supported authentication.
- Secure network communication.
- Session revocation.
- Device risk controls.
- Sensitive screen protection where appropriate.
- Application integrity checks.
- Secure update distribution.

Mobile-specific controls must be reviewed before native application release.

---

# 28. DATA PROTECTION AND ENCRYPTION

## SEC-DATA-001 — Encryption in Transit

Sensitive communication must use TLS.

## SEC-DATA-002 — Encryption at Rest

Sensitive databases, object storage and backups must use approved encryption.

## SEC-DATA-003 — Field-Level Protection

Highly sensitive fields may require application-level encryption.

## SEC-DATA-004 — Key Management

Encryption keys must be managed separately from encrypted data.

## SEC-DATA-005 — Data Minimization

Collect only necessary data.

## SEC-DATA-006 — Retention

Retention periods must follow approved legal and operational requirements.

## SEC-DATA-007 — Secure Deletion

Data must be securely deleted when permitted and required.

## SEC-DATA-008 — Access Logging

Sensitive data access must be logged where appropriate.

## SEC-DATA-009 — Data Classification

Data must be classified according to sensitivity.

## SEC-DATA-010 — Data Export

Customer data exports must be authenticated, authorized and protected.

---

# 29. KYC AND SENSITIVE IDENTITY DATA

## SEC-KYC-001 — Sensitive Identity Information

BVN, NIN and identity documents must receive enhanced protection.

## SEC-KYC-002 — Provider Minimization

Where possible, DEYPAY should store provider verification references rather than unnecessary raw identity documents.

## SEC-KYC-003 — Restricted Access

Only authorized personnel may access sensitive KYC information.

## SEC-KYC-004 — Audit Trail

KYC access and administrative decisions must be auditable.

## SEC-KYC-005 — Secure Upload

Document uploads must use controlled, authenticated mechanisms.

## SEC-KYC-006 — Malware Screening

Uploaded files must be validated and screened.

## SEC-KYC-007 — Retention

KYC retention must follow applicable legal requirements.

## SEC-KYC-008 — Data Protection Review

KYC processing must be included in the organization's privacy and data protection assessment.

---

# 30. DATABASE SECURITY

## SEC-DB-001 — Network Isolation

PostgreSQL must not be publicly exposed.

## SEC-DB-002 — Database Authentication

Database access must use secure credentials or approved identity-based authentication.

## SEC-DB-003 — Least Privilege

Database roles must have only required permissions.

## SEC-DB-004 — Financial Table Protection

Ledger tables must have restricted write permissions.

## SEC-DB-005 — Encryption

Database storage and backups must be encrypted.

## SEC-DB-006 — Auditability

Sensitive administrative database actions must be logged.

## SEC-DB-007 — Migration Controls

Production migrations must use controlled deployment procedures.

## SEC-DB-008 — Backup Testing

Database restoration must be tested.

## SEC-DB-009 — SQL Injection Protection

Parameterized queries must be used.

## SEC-DB-010 — Production Access

Direct production database access must be restricted and auditable.

---

# 31. INFRASTRUCTURE AND NETWORK SECURITY

## SEC-INF-001 — Network Segmentation

Public and private infrastructure must be separated.

## SEC-INF-002 — Private Database

The production database must reside on a private network.

## SEC-INF-003 — Edge Protection

Public endpoints must use appropriate edge security controls.

## SEC-INF-004 — DDoS Protection

The hosting architecture must provide appropriate denial-of-service mitigation.

## SEC-INF-005 — Patch Management

Infrastructure components must be patched according to risk.

## SEC-INF-006 — Hardened Images

Deployment images must use maintained and minimized base images.

## SEC-INF-007 — Service Identity

Internal services must authenticate where required.

## SEC-INF-008 — Administrative Access

Infrastructure administration must use MFA and least privilege.

## SEC-INF-009 — Configuration Management

Infrastructure configuration must be version-controlled where appropriate.

## SEC-INF-010 — Environment Isolation

Production and non-production environments must be separated.

---

# 32. SECRETS MANAGEMENT

## SEC-SEC-001 — Centralized Secrets

Production secrets must use an approved secrets management system.

## SEC-SEC-002 — No Source Code Secrets

Secrets must never be committed to source control.

## SEC-SEC-003 — No Frontend Secrets

Privileged provider credentials must never be exposed to frontend code.

## SEC-SEC-004 — Rotation

Secrets must support rotation.

## SEC-SEC-005 — Access Control

Secret access must be restricted by service identity.

## SEC-SEC-006 — Audit

Sensitive secret access must be auditable.

## SEC-SEC-007 — Incident Rotation

Compromised credentials must be revoked and replaced.

## SEC-SEC-008 — CI/CD Protection

Deployment pipelines must not expose production secrets in logs.

---

# 33. SOFTWARE SUPPLY CHAIN SECURITY

## SEC-SUP-001 — Dependency Review

Dependencies must be reviewed for security and maintenance.

## SEC-SUP-002 — Dependency Scanning

Automated vulnerability scanning must be integrated into CI.

## SEC-SUP-003 — Lockfiles

Dependency versions must be reproducible.

## SEC-SUP-004 — Build Integrity

Production builds must originate from controlled pipelines.

## SEC-SUP-005 — Artifact Integrity

Deployment artifacts must be traceable to approved source revisions.

## SEC-SUP-006 — Secret Scanning

Source repositories must be scanned for exposed secrets.

## SEC-SUP-007 — Code Review

Security-sensitive changes require peer review.

## SEC-SUP-008 — Release Approval

Production financial changes must follow controlled release procedures.

---

# 34. SECURE RUST DEVELOPMENT

## 34.1 Rust Security Approach

Rust provides memory safety advantages but does not eliminate application security vulnerabilities.

## SEC-RUST-001 — Safe Rust

Prefer safe Rust.

## SEC-RUST-002 — Unsafe Code

Unsafe Rust requires explicit justification and review.

## SEC-RUST-003 — Input Validation

All external input must be validated.

## SEC-RUST-004 — Error Handling

Errors must not expose sensitive information.

## SEC-RUST-005 — SQLx

Database operations must use parameterized queries.

## SEC-RUST-006 — Financial Types

Financial values must use exact representations.

## SEC-RUST-007 — Secret Handling

Secrets must not be unnecessarily copied, serialized or logged.

## SEC-RUST-008 — Dependency Auditing

Rust dependencies must be audited.

## SEC-RUST-009 — Concurrency Testing

Financial concurrency behavior must be tested.

## SEC-RUST-010 — Panic Handling

Unexpected panics must not leave financial operations in unrecoverable states.

---

# 35. ADMINISTRATIVE SECURITY

## SEC-ADM-001 — Separate Administrative Access

Administrative interfaces must have distinct access controls.

## SEC-ADM-002 — Strong MFA

Privileged administrators must use strong MFA.

## SEC-ADM-003 — Role Separation

Support, compliance, treasury and engineering roles must be separated.

## SEC-ADM-004 — Maker-Checker

High-risk financial actions must require independent approval.

## SEC-ADM-005 — No Direct Balance Editing

Administrators must not directly change customer balances.

## SEC-ADM-006 — Audit Logs

Administrative actions must be logged.

## SEC-ADM-007 — Privilege Review

Administrative permissions must be reviewed periodically.

## SEC-ADM-008 — Emergency Access

Emergency access must be time-limited, justified and audited.

## SEC-ADM-009 — Support Restrictions

Support personnel must not access private keys or privileged provider credentials.

## SEC-ADM-010 — Administrative Session Security

Administrative sessions must have stricter security controls than ordinary customer sessions.

---

# 36. TREASURY AND FINANCIAL OPERATIONS SECURITY

## SEC-TRY-001 — Treasury Separation

Treasury operations must be separated from ordinary customer support operations.

## SEC-TRY-002 — Approval Thresholds

High-value treasury movements must require additional approval.

## SEC-TRY-003 — Provider Funding Controls

Prefunding operations must be authorized and reconciled.

## SEC-TRY-004 — Beneficiary Restrictions

Treasury destinations must follow approved verification and allowlisting procedures.

## SEC-TRY-005 — Financial Reconciliation

Treasury balances must be reconciled with provider and bank evidence.

## SEC-TRY-006 — Liquidity Monitoring

Liquidity shortages and unusual movements must trigger alerts.

## SEC-TRY-007 — Emergency Suspension

The organization must be able to suspend affected treasury operations.

---

# 37. FRAUD PREVENTION AND TRANSACTION MONITORING

## 37.1 Fraud Risk Areas

DEYPAY must monitor:

- Account takeover.
- Identity fraud.
- Suspicious deposits.
- Rapid withdrawals.
- Unusual internal transfers.
- Beneficiary changes.
- High-frequency bank payouts.
- Structuring.
- Suspicious conversion patterns.
- Abuse of refunds.
- Social engineering.

## SEC-FRD-001 — Risk Engine

Financial operations must pass through an appropriate risk evaluation process.

## SEC-FRD-002 — Transaction Limits

Limits must be enforced server-side.

Apply shared limits atomically under 03-DATABASE-DESIGN.md §10.6. Pending and uncertain commitments count. Definitive non-execution releases unused capacity; executed amounts remain counted in their original window after reversal or refund.

## SEC-FRD-003 — Velocity Controls

The platform must support transaction velocity monitoring.

## SEC-FRD-004 — Device Risk

Device and session risk signals should be incorporated where lawful and appropriate.

## SEC-FRD-005 — Suspicious Activity

Suspicious activity must be escalated to authorized personnel.

## SEC-FRD-006 — Transaction Holds

The platform must support controlled transaction holds.

## SEC-FRD-007 — Manual Review

High-risk operations may require manual review.

## SEC-FRD-008 — Auditability

Risk decisions must be traceable.

## SEC-FRD-009 — Privacy

Fraud monitoring must comply with applicable privacy requirements.

---

# 38. LOGGING, MONITORING AND DETECTION

## SEC-LOG-001 — Structured Logging

Backend services must use structured logs.

## SEC-LOG-002 — Correlation IDs

Requests and financial operations must have correlation identifiers.

## SEC-LOG-003 — Sensitive Data Redaction

Secrets and sensitive identity information must not appear in ordinary logs.

## SEC-LOG-004 — Security Events

Log:

- Failed logins.
- Session revocations.
- MFA changes.
- Password resets.
- Financial authorization failures.
- Withdrawal attempts.
- Administrative changes.
- Provider authentication failures.
- Reconciliation discrepancies.

## SEC-LOG-005 — Tamper Resistance

Security logs must be protected against unauthorized alteration.

## SEC-LOG-006 — Alerting

Critical security events must trigger alerts.

## SEC-LOG-007 — Retention

Log retention must follow approved legal and operational policies.

## SEC-LOG-008 — Monitoring Access

Monitoring systems must use restricted access.

---

# 39. INCIDENT RESPONSE

## 39.1 Incident Categories

- Account compromise.
- Unauthorized financial movement.
- Provider compromise.
- Custody incident.
- Data breach.
- Infrastructure intrusion.
- Insider abuse.
- Ledger integrity incident.
- Payment settlement incident.
- Service outage.

## 39.2 Severity Levels

### SEV-1 — Critical

Examples:

- Confirmed unauthorized fund movement.
- Active custody compromise.
- Material ledger integrity failure.
- Major sensitive data breach.

### SEV-2 — High

Examples:

- Significant account takeover campaign.
- Major provider outage affecting financial settlement.
- Serious security vulnerability with active exploitation.

### SEV-3 — Medium

Examples:

- Contained security incident with limited impact.

### SEV-4 — Low

Examples:

- Minor security events without material impact.

## 39.3 Incident Response Lifecycle

1. Detection.
2. Triage.
3. Classification.
4. Containment.
5. Evidence preservation.
6. Investigation.
7. Eradication.
8. Recovery.
9. Reconciliation.
10. Post-incident review.

## SEC-IR-001 — Incident Ownership

Every incident must have an assigned incident commander.

## SEC-IR-002 — Emergency Controls

The organization must be able to suspend affected financial operations.

## SEC-IR-003 — Evidence Preservation

Relevant logs, transaction records and provider evidence must be preserved.

## SEC-IR-004 — Provider Coordination

Provider incidents must follow documented escalation procedures.

## SEC-IR-005 — Financial Reconciliation

Financial incidents must include reconciliation before normal operations resume.

## SEC-IR-006 — Notifications

Regulatory and customer notification obligations must be assessed according to applicable law and contractual requirements.

## SEC-IR-007 — Post-Incident Review

Material incidents must produce corrective actions.

---

# 40. BUSINESS CONTINUITY AND DISASTER RECOVERY

## SEC-BCP-001 — Recovery Planning

DEYPAY must maintain documented recovery procedures.

## SEC-BCP-002 — Backups

Critical databases and configurations must be backed up.

## SEC-BCP-003 — Backup Encryption

Backups must be encrypted.

## SEC-BCP-004 — Backup Isolation

Backups must be protected from compromise of primary production systems.

## SEC-BCP-005 — Restore Testing

Recovery procedures must be tested.

## SEC-BCP-006 — Financial Recovery

Restored financial state must be reconciled against external provider operations.

## SEC-BCP-007 — Recovery Objectives

Recovery time and recovery point objectives must be formally approved.

## SEC-BCP-008 — Provider Outage

Provider outages must place affected financial operations into safe states.

## SEC-BCP-009 — Communication

Operational incidents must have defined internal and customer communication procedures.

---

# 41. SECURITY TESTING AND ASSURANCE

## 41.1 Testing Categories

DEYPAY must perform:

- Unit security tests.
- Authentication tests.
- Authorization tests.
- API security tests.
- Financial abuse tests.
- Concurrency tests.
- Dependency scanning.
- Infrastructure configuration reviews.
- Penetration testing.
- Recovery testing.

## SEC-TST-001 — Authentication Testing

Verify protection against credential attacks and session abuse.

## SEC-TST-002 — Authorization Testing

Verify that users cannot access other users' resources.

## SEC-TST-003 — Financial Testing

Verify that unauthorized or duplicate financial operations cannot succeed.

## SEC-TST-004 — Webhook Testing

Verify rejection of forged and duplicate events.

## SEC-TST-005 — Provider Failure Testing

Verify safe handling of timeouts and uncertain outcomes.

## SEC-TST-006 — Penetration Testing

Independent penetration testing must be completed before production financial launch.

## SEC-TST-007 — Remediation

Critical and high-risk findings must be remediated or formally addressed before launch.

## SEC-TST-008 — Retesting

Material remediations must be verified.

---

# 42. VULNERABILITY MANAGEMENT

## SEC-VUL-001 — Vulnerability Intake

The organization must maintain a process for receiving vulnerability reports.

## SEC-VUL-002 — Classification

Vulnerabilities must be classified by severity and exploitability.

## SEC-VUL-003 — Ownership

Every material vulnerability must have an assigned owner.

## SEC-VUL-004 — Remediation

Remediation timelines must be defined by severity.

## SEC-VUL-005 — Emergency Patching

Critical exploited vulnerabilities must receive expedited treatment.

## SEC-VUL-006 — Verification

Remediated vulnerabilities must be retested.

---

# 43. THIRD-PARTY SECURITY GOVERNANCE

## 43.1 Provider Due Diligence

Before production integration, DEYPAY must assess:

- Provider legal identity.
- Security architecture.
- Custody responsibilities.
- API authentication.
- Data protection.
- Incident response.
- Business continuity.
- Financial reconciliation.
- Operational support.
- Contractual liability.

## 43.2 Nuvion-Specific Review

The Nuvion integration review must establish:

1. Which entity contracts with DEYPAY.
2. Which entity legally holds customer assets.
3. Which party controls signing authority.
4. Which security controls apply to withdrawals.
5. Whether DEYPAY can impose transaction policies.
6. How compromised credentials are revoked.
7. How provider incidents are reported.
8. How customer assets are recovered or migrated.
9. How provider balances are independently reconciled.
10. What evidence DEYPAY can obtain for audits.

## 43.3 Additional Providers

The same review principles apply to banking, KYC, conversion and payout providers.

## 43.4 Provider Risk Register

Material provider risks must be recorded and periodically reviewed.

---

# 44. REGULATORY AND COMPLIANCE DEPENDENCIES

## 44.1 Nigerian Regulatory Review

The operating model must undergo applicable Nigerian financial regulatory review before production launch.

## 44.2 Data Protection

Personal data processing must be reviewed against applicable Nigerian data protection requirements.

## 44.3 AML/CFT

Customer onboarding, transaction monitoring and reporting obligations must be determined under the applicable legal framework.

## 44.4 Custody

The legal custody arrangement must be approved.

## 44.5 Third-Party Payments

Pay Anyone is required for the MVP and can be built and tested against mocks or a sandbox. For live activation, Legal, Compliance and the contracting payment partners must document the entities, funding and settlement accounts, eligible recipients, permissions, safeguarding, reconciliation and recovery responsibilities for the selected route. Record the product owner's verbal guidance separately from formal operating evidence.

## 44.6 Regulatory Evidence

Security policies, testing records and operational controls must be retained as compliance evidence.

---

# 45. SECURITY NON-FUNCTIONAL REQUIREMENTS

## NFR-SEC-001 — Confidentiality

Sensitive information must be accessible only to authorized identities.

## NFR-SEC-002 — Integrity

Financial records must resist unauthorized modification.

## NFR-SEC-003 — Authentication

Protected operations must require valid authentication.

## NFR-SEC-004 — Authorization

Access must be evaluated server-side.

## NFR-SEC-005 — Auditability

Critical security and financial operations must produce traceable records.

## NFR-SEC-006 — Recoverability

The system must support controlled recovery after failures.

## NFR-SEC-007 — Provider Isolation

Provider compromise must not automatically compromise every DEYPAY subsystem.

## NFR-SEC-008 — Security Monitoring

Critical security events must be monitored.

## NFR-SEC-009 — Secure Development

Security checks must be integrated into development workflows.

## NFR-SEC-010 — Independent Assessment

Production financial launch requires independent security assessment.

---

# 46. SECURITY IMPLEMENTATION ROADMAP

## Phase 1 — Security Foundations

Implement:

- Secure Rust project configuration.
- Environment separation.
- Secret management.
- Structured logging.
- Authentication architecture.
- Authorization foundations.
- Database access restrictions.

## Phase 2 — Customer Security

Implement:

- Registration security.
- Login protection.
- Session management.
- MFA.
- Password recovery.
- Account restrictions.

## Phase 3 — Financial Security

Implement:

- Transaction authorization.
- Ledger posting restrictions.
- Idempotency.
- Atomic reservations.
- Transaction limits.
- Risk controls.

## Phase 4 — Provider Security

Implement:

- Provider credential isolation.
- Authenticated API communication.
- Webhook verification.
- Provider operation tracking.
- Safe retries.

## Phase 5 — Administrative Security

Implement:

- Administrative RBAC.
- Strong MFA.
- Maker-checker.
- Audit trails.
- Emergency controls.

## Phase 6 — Infrastructure Security

Implement:

- Private database networking.
- Edge protection.
- Encrypted backups.
- Monitoring.
- Deployment controls.

## Phase 7 — Security Validation

Complete:

- Automated security tests.
- Financial abuse tests.
- Penetration testing.
- Recovery drills.
- Provider security reviews.
- Remediation verification.

---

# 47. SECURITY ACCEPTANCE CRITERIA

The security architecture will be accepted when:

1. Authentication controls are implemented and tested.
2. Customer sessions can be revoked.
3. Protected APIs enforce authorization.
4. Customer data is isolated.
5. Financial operations require appropriate authorization.
6. Transaction authorization is bound to the correct details.
7. Duplicate financial execution is prevented.
8. Ledger postings are controlled and auditable.
9. Provider credentials are protected.
10. Webhooks are authenticated and deduplicated.
11. Custody responsibilities are documented.
12. Administrative access is restricted.
13. High-risk financial actions require appropriate approval.
14. Sensitive data is encrypted.
15. Production databases are privately accessible.
16. Security logs are protected.
17. Incident response procedures are documented.
18. Backups can be restored.
19. Provider security due diligence is completed.
20. Independent security testing is completed.
21. Critical security findings are resolved.
22. Applicable regulatory launch requirements are satisfied.

---

# 48. OPEN DECISIONS AND DEPENDENCIES

The following must be finalized before production launch:

1. Authentication provider or internal authentication implementation.
2. MFA methods.
3. Session expiration policy.
4. Step-up authorization mechanism.
5. Account recovery policy.
6. Withdrawal risk thresholds.
7. Transaction limits.
8. Custody provider security responsibilities.
9. Nuvion credential and signing permissions.
10. Provider incident escalation procedures.
11. Production cloud provider.
12. Secrets management solution.
13. Logging and monitoring platform.
14. Backup and recovery objectives.
15. Administrative access policy.
16. Penetration testing provider.
17. Security incident notification procedures.
18. Applicable Nigerian regulatory security requirements.
19. Data retention and deletion policy.
20. Production security sign-off authority.

---

# 49. FINAL SECURITY ARCHITECTURE DEFINITION

DEYPAY will implement a defense-in-depth security architecture across customer applications, backend services, financial operations, provider integrations, databases and infrastructure.

The Rust backend will enforce authentication, authorization, financial transaction rules and access controls.

Customer balances will be protected through the independent double-entry ledger and controlled posting mechanisms.

External stablecoin withdrawals and Nigerian bank payments will require explicit authorization, sufficient available funds, valid transaction parameters and approved provider execution.

Nuvion and other providers will operate behind DEYPAY-controlled integration boundaries.

Provider credentials will remain isolated from customer applications.

Sensitive customer information will be protected through encryption, restricted access, retention controls and auditability.

Administrative financial operations will require appropriate separation of duties and approval.

Security monitoring, incident response and financial reconciliation will operate continuously throughout the production lifecycle.

No financial feature may bypass security controls merely to improve speed or user convenience.

**FINAL SECURITY PRINCIPLE**

A customer account must not be sufficient to bypass financial authorization.

A frontend request must not be sufficient to execute a payout.

A provider webhook must not be sufficient to create an unverified financial credit.

An administrator must not be sufficient to silently alter the ledger.

A provider integration must not be sufficient to take control of DEYPAY's internal financial system.

Every sensitive operation must be authenticated, authorized, validated, recorded and independently verifiable.

---

# 50. APPROVAL AND SIGN-OFF

| Role | Approval Requirement | Status |
|---|---|---|
| Product Owner | Business approval | Pending |
| Engineering Lead | Technical approval | Pending |
| Security Lead | Security architecture approval | Pending |
| Financial Operations Lead | Financial control approval | Pending |
| Compliance Lead | Compliance approval | Pending |
| Legal Counsel | Legal and regulatory review | Pending |
| Independent Security Assessor | Security testing approval | Pending |

**Document Status:** Proposed Engineering Baseline.

Approval must be based on implementation evidence and completed reviews.

---

**END OF DOCUMENT**

**DEYPAY SECURITY ARCHITECTURE AND THREAT MODEL**

**Document ID:** DP-SEC-006  
**Version:** 1.0  
**Status:** Proposed Engineering Baseline — Pending Final Validation