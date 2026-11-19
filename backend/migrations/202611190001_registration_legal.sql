-- Immutable, versioned legal text. The four source files are snapshotted below
-- so a fresh database and historical document links use identical UTF-8/LF bytes.
CREATE TABLE customers.legal_document_revisions (
    document_id text NOT NULL, version text NOT NULL CHECK (btrim(version) NOT IN ('','.','..')),
    edition text NOT NULL DEFAULT 'draft' CHECK (edition IN ('draft','final')),
    title text NOT NULL, content bytea NOT NULL CHECK (octet_length(content) > 0),
    sha256 text GENERATED ALWAYS AS (encode(sha256(content),'hex')) STORED,
    PRIMARY KEY (document_id,version), UNIQUE (document_id,version,edition)
);
CREATE TRIGGER immutable_legal_revision BEFORE UPDATE OR DELETE OR TRUNCATE
    ON customers.legal_document_revisions FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

-- One submitted version pair identifies exactly one immutable set of checkbox wording in each scope.
CREATE TABLE customers.registration_legal_releases (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    scope text NOT NULL CHECK (scope IN ('sandbox','published')),
    edition text GENERATED ALWAYS AS (CASE scope WHEN 'published' THEN 'final' ELSE 'draft' END) STORED,
    terms_id text NOT NULL DEFAULT 'DEP-LEG-001' CHECK (terms_id = 'DEP-LEG-001'),
    privacy_id text NOT NULL DEFAULT 'DEP-LEG-002' CHECK (privacy_id = 'DEP-LEG-002'),
    wording_id text NOT NULL DEFAULT 'DEP-LEG-003' CHECK (wording_id = 'DEP-LEG-003'),
    terms_version text NOT NULL, privacy_version text NOT NULL, wording_version text NOT NULL,
    approval_reference text, published_at timestamptz,
    CHECK ((scope='sandbox' AND published_at IS NULL AND approval_reference IS NULL)
        OR (scope='published' AND published_at IS NOT NULL AND approval_reference IS NOT NULL
            AND btrim(approval_reference) <> '')),
    FOREIGN KEY (terms_id,terms_version,edition) REFERENCES customers.legal_document_revisions(document_id,version,edition),
    FOREIGN KEY (privacy_id,privacy_version,edition) REFERENCES customers.legal_document_revisions(document_id,version,edition),
    FOREIGN KEY (wording_id,wording_version,edition) REFERENCES customers.legal_document_revisions(document_id,version,edition),
    UNIQUE (scope,terms_version,privacy_version), UNIQUE (scope,id), UNIQUE (id,terms_version,privacy_version)
);
CREATE TRIGGER immutable_legal_release BEFORE UPDATE OR DELETE OR TRUNCATE
    ON customers.registration_legal_releases FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

CREATE TABLE customers.current_registration_release (
    scope text PRIMARY KEY, release_id bigint NOT NULL,
    FOREIGN KEY (scope,release_id) REFERENCES customers.registration_legal_releases(scope,id)
);
CREATE VIEW customers.registration_documents AS
    SELECT r.scope, (c.release_id IS NOT NULL) AS current, d.document_id,d.version,d.title,d.sha256,d.content
    FROM customers.registration_legal_releases r
    CROSS JOIN LATERAL (VALUES (r.terms_id,r.terms_version),(r.privacy_id,r.privacy_version),
                               (r.wording_id,r.wording_version)) v(document_id,version)
    JOIN customers.legal_document_revisions d USING (document_id,version)
    LEFT JOIN customers.current_registration_release c ON (c.scope,c.release_id)=(r.scope,r.id)
    WHERE r.scope='sandbox' OR r.published_at <= statement_timestamp();

INSERT INTO customers.legal_document_revisions(document_id,version,title,content) VALUES
('DEP-LEG-003','1.0','DEYPAY CUSTOMER CONSENT & ACKNOWLEDGEMENTS',convert_to($legal$# DEYPAY CUSTOMER CONSENT & ACKNOWLEDGEMENTS

**Document ID:** DEP-LEG-003  
**Version:** 1.0 — Draft for Legal Approval  
**Related Documents:** DEP-LEG-001 and DEP-LEG-002  
**Application:** Customer Registration

## 1. Purpose

This document defines the customer-facing acknowledgements and optional consent choices presented during DEYPAY account registration.

DEYPAY is a product of Kenule Africa.

The purpose is to ensure customers understand the legal documents governing their accounts, the relevant processing of personal data and the restrictions applicable before identity verification and financial activation.

## 2. Required Acceptance of Terms

**Checkbox label — required; unchecked by default**

[ ] I have read and agree to the DEYPAY Terms of Service (Version 1.0).

**Supporting explanation**

By accepting, I agree to the terms governing my DEYPAY account. I understand that creating an account does not automatically make me eligible to deposit, withdraw, convert or transfer digital assets or make bank payments.

## 3. Required Privacy Notice Acknowledgement

**Checkbox label — required; unchecked by default**

[ ] I have read and understood the DEYPAY Privacy Notice (Version 1.0).

**Supporting explanation**

I understand that DEYPAY collects and processes information necessary to create and secure my account, verify my identity, provide requested services and comply with applicable law.

I understand that the Privacy Notice describes the applicable purposes and legal bases, categories of recipients, retention arrangements and my rights.

This acknowledgement is evidence that the Notice was presented and acknowledged. It is not blanket consent to all possible personal-data processing.

## 4. Customer Identity and KYC Information

**Information displayed during onboarding; not a blanket consent checkbox**

DEYPAY manages customer identity verification and KYC for its platform.

When I request features requiring identity verification, DEYPAY may ask for additional identification information and documents.

DEYPAY may use approved verification technology providers and conduct legally required compliance checks.

Where information must be shared with a banking, custody or payment infrastructure provider, DEYPAY will apply the relevant lawful basis and provide required disclosures.

Any processing that legally requires separate consent, including particular biometric processing where applicable, will be presented through a separate, specific consent mechanism before that processing takes place.

## 5. Age and Accuracy Confirmation

**Checkbox label — required if adopted as a contractual representation**

[ ] I confirm that I am at least 18 years old, that I am authorized to register, and that the information I have provided is accurate.

Where age eligibility is verified later, DEYPAY may restrict the account if the information does not satisfy the applicable requirements.

## 6. Electronic Communications

**Required service information**

DEYPAY may send messages necessary for email verification, phone verification, authentication, account security, customer support and relevant service communications.

These communications are necessary for operating the requested service or meeting applicable obligations and are not conditional on optional marketing consent.

## 7. Optional Marketing Consent

**Checkbox label — optional; unchecked by default**

[ ] I would like to receive optional DEYPAY product news, feature announcements and promotional messages by email or other channels I select.

I understand that I can withdraw this consent at any time through the published preference or unsubscribe process.

Refusing this optional consent will not prevent me from registering for DEYPAY.

Separate choices must be provided where legally required for different channels or marketing purposes.

## 8. Optional Analytics and Tracking

Where DEYPAY deploys optional analytics, advertising cookies, software development kits or tracking technologies requiring consent, the application must display an appropriate separate notice and choice.

These controls must not be silently combined with mandatory registration acceptance.

## 9. Consent Withdrawal and Preference Management

Customers must be able to withdraw optional consent through an accessible mechanism.

Withdrawal will apply to future processing based on that consent, subject to legally permitted limitations.

Withdrawal of optional consent does not automatically terminate the customer agreement or erase information that DEYPAY has another lawful basis to retain.

## 10. Registration Confirmation

Before submission, the application must confirm that:

- Mandatory customer fields have been completed.
- The customer has affirmatively accepted the current Terms.
- The customer has acknowledged the current Privacy Notice.
- The applicable age and eligibility declaration has been completed, if required.
- Optional marketing selections have not been preselected.
- The legal document links are accessible before submission.

## 11. Recordkeeping

DEYPAY will record the versions of the applicable documents, the customer's affirmative actions and the necessary evidence that the registration acknowledgements were made.

Optional consent decisions must be recorded separately from mandatory contractual acceptance.

Historical records must be retained and protected according to the approved retention policy.

**Status:** Draft — the exact UI wording, legal versions and any processing-specific consent must be approved before public registration is enabled.$legal$,'UTF8')),
('DEP-LEG-002','1.0','DEYPAY PRIVACY NOTICE',convert_to($legal$# DEYPAY PRIVACY NOTICE

**Document ID:** DEP-LEG-002  
**Version:** 1.0 — Draft for Legal Approval  
**Effective Date:** To be assigned upon approval  
**Product:** DEYPAY  
**Parent Organization:** Kenule Africa  
**Data Controller:** [Insert verified registered legal name of the operating entity]  
**Registered Address:** [Insert]  
**Privacy Contact:** [Insert official privacy contact email]

## 1. Introduction

DEYPAY, a financial technology product under Kenule Africa, is committed to protecting the privacy, confidentiality and security of its customers' personal information.

This Privacy Notice explains how we collect, use, disclose, store and protect personal data when you register for or use DEYPAY.

It also explains the rights available to you under applicable data-protection laws, including the Nigeria Data Protection Act 2023 and relevant regulatory directives.

For the personal-data processing activities described in this Notice, the operating entity identified above is responsible for determining the purposes and essential means of processing, except where another organization independently determines those matters under applicable law.

## 2. Scope

This Notice applies to personal data collected through:

- DEYPAY's website and customer registration process.
- DEYPAY's web and mobile applications.
- Account authentication and security systems.
- Identity verification and KYC processes.
- Customer support interactions.
- Financial transactions where the relevant service is approved and active.
- Security, fraud prevention and compliance activities.
- Related communications and approved integrations.

This Notice does not automatically cover unrelated websites or services operated independently by third parties.

## 3. Personal Data We Collect

### 3.1 Registration and Account Information

We may collect:

- First name and last name.
- Email address.
- Phone number.
- Country of residence.
- Username.
- Internal customer identifier.
- Account creation date.
- Applicable terms and privacy acknowledgement records.

### 3.2 Authentication and Security Information

We may process:

- Secure password hashes.
- Authentication events.
- Verification challenges and their status.
- Account sessions and device identifiers.
- Relevant IP addresses, user agents and security logs.
- Failed authentication attempts.
- Account restrictions and security alerts.

Passwords must be protected using approved password hashing methods. DEYPAY does not intentionally store customer passwords in plaintext.

### 3.3 Identity Verification and KYC Information

Where required for an applicable service, we may collect and process:

- Full legal name.
- Date of birth.
- Nationality and country of residence.
- Residential address.
- Identification document details.
- Verification evidence and document images.
- Government-issued identifiers where lawfully required.
- Selfies, identity comparison information or biometric information where necessary and lawfully permitted.
- Verification outcomes.
- Sanctions, politically exposed person and compliance screening results.
- Customer risk classification and review records.

Sensitive or biometric data will be processed only where an appropriate lawful basis, security safeguards and other applicable legal conditions exist.

DEYPAY determines and manages its customer KYC process. Approved specialist vendors may support verification technology, but infrastructure partners do not automatically control DEYPAY's customer identity system.

### 3.4 Financial and Transaction Information

If financial features become available, we may process:

- Supported asset and wallet identifiers.
- Blockchain network and transaction references.
- Deposit and withdrawal records.
- Transaction instructions, amounts and statuses.
- Beneficiary information.
- Exchange rate quotations and accepted transaction terms.
- Banking or payment references.
- Settlement and reconciliation information.
- Financial compliance and transaction-monitoring records.

DEYPAY collects such information only when the relevant feature and lawful processing purpose apply.

### 3.5 Technical and Usage Information

We may collect device, browser, application, diagnostic and network information reasonably needed to operate, maintain and secure the service.

Any optional analytics or tracking technology requiring consent will be subject to a separate, appropriate consent mechanism.

### 3.6 Customer Communications

We may retain customer service requests, complaints, support correspondence, notification preferences and records of important customer communications.

## 4. Purposes and Lawful Bases

We process personal data for the purposes and lawful bases identified below. Applicability depends on the feature and the processing activity.

| Purpose | Expected lawful basis |
|---|---|
| Creating and administering customer accounts | Performance of a contract or steps requested before entering one |
| Authentication and account access | Contractual necessity and applicable legitimate interests |
| Mandatory KYC, sanctions screening and regulatory reporting | Compliance with an applicable legal obligation, where the obligation applies |
| Fraud detection and service security | Legitimate interests or applicable legal obligation, subject to required safeguards |
| Delivering requested customer notifications | Contractual necessity or other applicable lawful basis |
| Customer support and complaint handling | Contractual necessity, legitimate interests or legal obligation, depending on the activity |
| Optional product marketing | Consent where required |
| Optional nonessential tracking | Consent where required |
| Financial transaction processing | Contractual necessity and applicable regulatory obligations |

Where we rely on legitimate interests, we will assess whether those interests are appropriate and do not override customers' rights and freedoms.

Where we rely on consent, we will seek a freely given, specific, informed and unambiguous affirmative indication.

Accepting our Terms of Service or acknowledging this Privacy Notice does not by itself constitute consent to every possible use of personal data.

## 5. How We Use Personal Data

DEYPAY uses personal information to:

1. Create and maintain customer accounts.
2. Authenticate customers and protect account access.
3. Verify email addresses and phone numbers.
4. Carry out customer identity and KYC checks.
5. Determine eligibility for available services.
6. Process authorized financial instructions where enabled.
7. Detect, investigate and prevent fraud and unauthorized activity.
8. Meet applicable legal and regulatory requirements.
9. Investigate complaints and resolve disputes.
10. Operate, maintain and improve service security and reliability.
11. Send necessary operational and security communications.
12. Send optional marketing messages only where lawfully permitted.

We will not use personal information for unrelated purposes without an appropriate lawful basis and required notice.

## 6. DEYPAY and External Service Providers

DEYPAY controls its own customer-facing authentication, identity management and KYC processes.

We may engage external providers for specific functions, including:

- Cloud hosting and database infrastructure.
- Email, SMS and notification delivery.
- Identity document and verification technology.
- Security and fraud detection services.
- Banking and payment infrastructure.
- Digital asset custody infrastructure.
- Liquidity, exchange and settlement services, where approved.
- Customer support and operational technology.

A provider's access must be limited to the information reasonably necessary for its assigned function.

Where a provider processes data on our instructions, appropriate data-processing agreements and safeguards will apply.

Where a bank, custodian or other financial institution independently processes information to meet its own legal responsibilities, it may act as an independent data controller for that processing. Its privacy obligations and notices may also apply.

We will identify material provider-specific disclosures as required by law.

DEYPAY does not sell customer personal data to advertisers.

## 7. Disclosure of Personal Information

We may disclose personal data:

- To authorized service providers performing approved functions.
- To relevant financial infrastructure providers where a customer requests an enabled service.
- To lawful regulatory, supervisory or enforcement authorities where required.
- To professional advisers under appropriate confidentiality arrangements.
- To parties necessary to investigate fraud, security incidents or disputes.
- In connection with a lawful corporate restructuring, subject to applicable protection requirements.

We do not provide blanket access to customer KYC files or authentication secrets to banking or custody partners.

Disclosures will be limited by lawful purpose, necessity and applicable regulatory requirements.

## 8. Cross-Border Data Transfers

Some approved infrastructure or technology providers may process or store information outside Nigeria.

Before transferring personal data internationally, DEYPAY will assess the applicable transfer rules and implement a lawful transfer mechanism and required safeguards.

These may include appropriate contractual protections, approved transfer arrangements or other legally permitted mechanisms.

The relevant countries, provider roles and transfer safeguards must be documented in DEYPAY's internal data-processing register and disclosed where required.

## 9. Data Security

DEYPAY will maintain technical and organizational measures appropriate to the information and risks involved.

Measures may include:

- Password hashing and credential protection.
- Encryption in transit and appropriate encryption at rest.
- Access control and least-privilege permissions.
- Multifactor authentication where applicable.
- Security logging and monitoring.
- Restricted access to KYC records.
- Protected backups.
- Incident detection and response.
- Employee and service-provider confidentiality controls.
- Periodic security assessments.

No system can be guaranteed completely free of risk, but DEYPAY remains responsible for implementing safeguards required by applicable law.

## 10. Data Retention

We retain personal information only for as long as reasonably necessary for the purposes described in this Notice or as otherwise required or permitted by law.

Retention depends on the data category:

- Registration and account records: while the account remains active and for any applicable post-closure obligations.
- Authentication and security logs: according to the approved security retention schedule.
- KYC and financial compliance records: for legally required periods applicable to the service and entity.
- Transaction records: for applicable financial, accounting, compliance and dispute-resolution periods.
- Legal acknowledgement records: for as long as necessary to evidence applicable agreements, lawful processing and potential claims.
- Marketing consent records: until no longer necessary to administer consent or demonstrate compliance, subject to lawful retention.

The exact category-by-category retention periods will be published or otherwise provided in the approved retention schedule before production activation.

When information is no longer required, it will be securely deleted, anonymized or otherwise handled in accordance with applicable law.

## 11. Your Privacy Rights

Subject to applicable law and lawful exceptions, you may have rights to:

1. Obtain information about how your personal data is processed.
2. Request access to relevant personal data.
3. Request correction of inaccurate or incomplete information.
4. Request deletion where applicable.
5. Withdraw consent for processing based on consent.
6. Object to certain processing activities.
7. Request restrictions or other remedies where available.
8. Receive personal data in an applicable portable format.
9. Challenge certain solely automated decisions that significantly affect you.
10. Lodge a complaint with the Nigeria Data Protection Commission.

Withdrawal of optional consent will not invalidate processing already lawfully carried out and will not automatically prevent processing that has another valid legal basis.

Requests may be subject to identity verification and applicable legal retention or disclosure restrictions.

## 12. Automated Decisions and Risk Assessment

DEYPAY may use automated controls for security checks, fraud detection, transaction monitoring and account-risk assessment.

Such controls may identify activities requiring additional verification or human review.

Where a decision is based solely on automated processing and produces legal or similarly significant effects, DEYPAY will comply with applicable requirements concerning transparency, permitted grounds, safeguards and rights to challenge the decision or obtain human intervention.

The use, categories, significance and consequences of such decisions must be accurately described when relevant systems are activated.

## 13. Children and Age Restrictions

DEYPAY's initial registration policy is intended for individuals aged 18 years and above.

DEYPAY does not knowingly offer account registration to children under its approved eligibility policy.

Where we discover that a person is ineligible due to age, we will take appropriate steps consistent with applicable legal and data-protection obligations.

## 14. Security Incidents and Data Breaches

DEYPAY will maintain procedures for identifying, investigating, containing and responding to personal-data incidents.

Where a personal-data breach triggers a legal reporting or notification obligation, DEYPAY will notify the relevant authority and affected individuals as required by applicable law.

## 15. Marketing Choices

Customers may choose whether to receive optional DEYPAY promotional communications.

Marketing consent will be separate from acceptance of the Terms of Service and acknowledgement of this Privacy Notice.

Customers may withdraw marketing consent through the available preference controls or other published withdrawal channel.

Necessary account, security, compliance and transaction-related messages are not treated as optional marketing.

## 16. Account Closure and Data Deletion

Customers may request account closure or applicable deletion of personal data through DEYPAY's published process.

Closure or deletion requests may be subject to lawful retention duties, pending complaints, fraud investigations, outstanding transactions or legal proceedings.

We will explain any applicable limitations when responding to a request.

## 17. Updates to This Notice

DEYPAY may revise this Notice where services, processing activities, provider arrangements or legal requirements change.

Material revisions will be communicated as required by law.

Where a new processing activity requires separate consent, DEYPAY will request it before relying on that consent.

Historical versions will be maintained for accountability.

## 18. Contacting DEYPAY

**Data Controller:** [Verified Kenule Africa operating entity]  
**Product:** DEYPAY  
**Registered Address:** [Insert]  
**Privacy Email:** [Insert]  
**Data Protection Officer / Designated Privacy Contact:** [Insert, where applicable]  
**Customer Support:** [Insert]

You may also contact the Nigeria Data Protection Commission regarding your applicable statutory rights.

**Status:** Draft — subject to approval of the entity details, data inventory, lawful-basis assessment, retention schedule, cross-border transfer arrangements and operational safeguards.$legal$,'UTF8')),
('DEP-LEG-004','1.0','DEYPAY REGISTRATION CONSENT & ACCEPTANCE RECORD',convert_to($legal$# DEYPAY REGISTRATION CONSENT & ACCEPTANCE RECORD

**Document ID:** DEP-LEG-004  
**Version:** 1.0 — Engineering and Compliance Draft  
**Record Owner:** DEYPAY  
**Parent Organization:** Kenule Africa  
**Responsible Functions:** Product, Engineering, Security, Legal and Compliance

## 1. Objective

Establish a secure, reliable and auditable record of customer acceptance of DEYPAY's Terms of Service, acknowledgement of its Privacy Notice and separate optional consent decisions.

This specification applies to DEYPAY customer registration and subsequent material changes requiring renewed acceptance.

DEYPAY is responsible for generating and maintaining its own registration and acceptance evidence.

A banking, custody, liquidity or settlement provider must not be treated as the source of truth for DEYPAY's contractual acceptance or authentication records.

## 2. Approved Registration Information

The product owner has approved collecting the following customer information during registration:

- First name.
- Last name.
- Email address.
- Phone number.
- Country code.
- Username.
- Password.
- Terms of Service version.
- Privacy Notice version.
- Explicit acceptance of the Terms.
- Explicit acknowledgement of the Privacy Notice.

The application must not omit first name, last name or country of residence from the approved registration process.

Passwords are collected securely for credential creation and must not be stored in plaintext or included in acceptance audit records.

## 3. Acceptance Evidence Requirements

Each accepted registration must preserve sufficient evidence to identify:

| Field | Description |
|---|---|
| Acceptance event ID | Unique identifier for the acceptance event |
| Customer ID | Immutable DEYPAY internal customer identifier |
| Event type | Registration acceptance or later renewed acceptance |
| Terms document ID | DEP-LEG-001 |
| Terms version | Exact approved version accepted |
| Terms content digest | Integrity reference to the published version |
| Terms accepted | Affirmative true |
| Privacy document ID | DEP-LEG-002 |
| Privacy version | Exact approved version acknowledged |
| Privacy content digest | Integrity reference to the published version |
| Privacy acknowledged | Affirmative true |
| Accepted timestamp | Trusted server-generated UTC timestamp |
| Recorded timestamp | Trusted database timestamp |
| Channel | Web, mobile or another approved channel |
| Registration request ID | Correlation identifier for the submission |
| Acceptance wording version | Identifier of the actual checkbox wording shown |
| Legal document publication IDs | References to the exact published documents |
| Client/network evidence | Limited IP/device context where proportionate and lawfully retained |
| Record integrity | Immutable event, append-only controls or equivalent tamper-evidence |

The system must be able to establish which documents and wording were presented when acceptance occurred.

## 4. Required Recording Rules

1. Terms acceptance and Privacy Notice acknowledgement must use distinct, unchecked-by-default controls.
2. Both actions must be affirmative before a registration succeeds.
3. The backend must verify that submitted versions match currently approved and published documents.
4. A timestamp submitted by the customer must not be treated as authoritative acceptance time.
5. The server must generate the authoritative acceptance timestamp.
6. The customer must not select arbitrary document versions to bypass current requirements.
7. Registration and its acceptance records must be committed atomically.
8. A failed registration must not leave an orphaned customer or misleading completed acceptance event.
9. Subsequent material updates must create new acceptance events rather than overwrite historical records.
10. The system must prevent unauthorized modifications or deletions.
11. The acceptance mechanism must not allow a customer to impersonate another customer.
12. Optional consent must be stored independently from mandatory acceptance.

## 5. Relationship to Customer Registration

Once a registration request passes validation, the system may create:

- An immutable customer ID.
- A customer profile.
- A normalized username claim.
- An Argon2id password credential.
- A versioned Terms acceptance record.
- A Privacy Notice acknowledgement record.
- An audit event.
- Email and phone verification requests or corresponding outbox events.

These actions should succeed or fail as one atomic business operation.

The newly created account must remain unverified.

Registration must not automatically create an authenticated session, provision a custodial wallet, authorize transactions or activate regulated financial services.

## 6. Optional Consent Event Model

Where a lawful processing purpose relies on consent, each decision must identify:

- Consent event ID.
- Customer ID.
- Specific processing purpose.
- Consent wording and version.
- Consent status (granted, refused or withdrawn).
- Timestamp.
- Channel.
- Withdrawal timestamp, if applicable.
- Evidence and lawful retention references.

Refusal or withdrawal of optional marketing consent must not prevent account registration.

The system must support a verifiable history of consent changes.

## 7. Record Access and Security

Access to registration acceptance records should be restricted to authorized DEYPAY personnel and system components with a legitimate need.

Customer support must not receive unnecessary access to password hashes, authentication secrets or sensitive verification materials.

Security and compliance access should be logged where appropriate.

Acceptance records should be protected against unauthorized alteration and retained according to the approved data-retention schedule.

Any disclosure to regulators or professional advisers must follow applicable law and internal authorization rules.

## 8. Verification and Testing Requirements

Automated tests must verify:

- Registration succeeds with valid approved document versions and explicit acknowledgements.
- Missing Terms acceptance is rejected.
- Missing Privacy Notice acknowledgement is rejected.
- False acceptance flags are rejected.
- Unsupported or expired document versions are rejected.
- Client-supplied acceptance timestamps cannot override server time.
- Duplicate submissions do not create duplicate customers or conflicting acceptance events.
- Concurrent registrations preserve username uniqueness and transaction atomicity.
- Database failure does not leave partial acceptance records.
- A customer cannot alter another customer's acceptance records.
- Historical records remain immutable.
- Optional marketing consent is genuinely optional.
- Production registration remains disabled where required legal versions or security configuration are absent.

## 9. Example Acceptance Audit Event

The following is an illustrative internal record, not a required exact database schema:

```json
{
  "event_type": "REGISTRATION_LEGAL_ACCEPTANCE",
  "event_id": "generated-uuid",
  "customer_id": "generated-customer-uuid",
  "registration_request_id": "generated-request-id",
  "terms": {
    "document_id": "DEP-LEG-001",
    "version": "1.0",
    "accepted": true,
    "published_content_digest": "approved-document-sha256"
  },
  "privacy": {
    "document_id": "DEP-LEG-002",
    "version": "1.0",
    "acknowledged": true,
    "published_content_digest": "approved-document-sha256"
  },
  "accepted_at": "server-generated-utc-timestamp",
  "channel": "WEB",
  "acceptance_wording_version": "1.0",
  "account_status": "CONTACT_UNVERIFIED"
}
```

This audit event must never contain the plaintext password.

## 10. Document Governance

The Terms, Privacy Notice and customer-facing acknowledgement wording must be approved through the designated legal and product review process.

The approved document files must have stable version identifiers and integrity references.

Material legal updates requiring renewed customer acceptance must generate new historical acceptance events.

The application must not silently replace previously accepted versions or treat continued usage as affirmative consent where applicable law requires an explicit action.

## 11. Approval and Release Requirements

Before enabling public registration, the responsible teams must establish:

- Verified legal operator and contact details.
- Approved Terms and Privacy Notice.
- Published and accessible document URLs.
- Approved version identifiers and document integrity references.
- Final required registration fields and acknowledgement language.
- Production security configuration.
- Tested backend validation and acceptance persistence.
- Approved access controls and audit evidence.
- Relevant lawful-basis and retention arrangements.
- Successful registration security and integration tests.

**Status:** Engineering and compliance draft. The approved registration fields are a product decision, but final legal publication and security activation require their respective approvals.$legal$,'UTF8')),
('DEP-LEG-001','1.0','DEYPAY TERMS OF SERVICE',convert_to($legal$# DEYPAY TERMS OF SERVICE

**Document ID:** DEP-LEG-001  
**Version:** 1.0 — Draft for Legal Approval  
**Effective Date:** To be assigned upon approval  
**Product:** DEYPAY  
**Parent Organization:** Kenule Africa  
**Contracting Entity:** [Insert verified registered legal name of the Kenule Africa operating entity]  
**Registered Address:** [Insert verified registered address]  
**Support Contact:** [Insert official customer support email]

## 1. Introduction and Acceptance

Welcome to DEYPAY, a financial technology platform developed and operated under Kenule Africa.

These Terms of Service ("Terms") govern your registration for, access to and use of the DEYPAY website, mobile applications, customer accounts and services made available through the platform.

By creating a DEYPAY account and affirmatively accepting these Terms, you enter into an agreement with the legal entity identified above ("DEYPAY", "we", "us" or "our").

Please read these Terms and the DEYPAY Privacy Notice before registering.

If you do not agree to these Terms, you must not create or use a DEYPAY account.

## 2. About DEYPAY

DEYPAY is a digital financial services product designed to provide account management and, where legally permitted and operationally available, access to supported stablecoin and payment services.

DEYPAY is responsible for its own customer-facing application, customer account management, authentication systems, KYC processes, authorization controls, transaction instructions, internal records, customer support and applicable compliance responsibilities.

Certain financial capabilities may depend on approved third-party providers, including banking institutions, payment service providers, digital asset custodians, liquidity providers and settlement infrastructure.

Such providers supply the relevant infrastructure or regulated service. Their involvement does not automatically transfer ownership of DEYPAY customer accounts or DEYPAY's authentication and KYC systems to them.

Where a provider must independently perform legally required checks, DEYPAY may exchange the minimum necessary information with that provider under applicable law and approved arrangements.

**Registration does not mean that any stablecoin, banking, custody, conversion or payout service is currently available or approved for use.** Available services will be identified in the application.

## 3. Eligibility

To register for DEYPAY, you must:

1. Be at least 18 years old, unless a different minimum age is lawfully approved and expressly published by DEYPAY.
2. Have the legal capacity to enter into these Terms.
3. Provide accurate, complete and current registration information.
4. Use an email address and phone number you are authorized to access.
5. Be located in a jurisdiction supported by DEYPAY.
6. Not be prohibited by applicable law, sanctions or legally binding restrictions from using the services.

DEYPAY may request additional information to determine account eligibility.

## 4. Account Registration

Registration may require your first name, last name, email address, phone number, country of residence, username, password and acceptance of applicable legal documents.

You agree to provide truthful information and update it when necessary.

DEYPAY may reject, suspend or restrict registration where information is invalid, duplicated, fraudulent or inconsistent with applicable requirements.

A successfully created account is initially unverified and may have restricted functionality.

Registration does not automatically establish eligibility to hold, transfer or convert digital assets or to initiate bank payouts.

## 5. Authentication and Account Security

DEYPAY manages customer authentication and access control through its own systems.

You are responsible for protecting your password, devices and authentication credentials and for promptly notifying DEYPAY of suspected unauthorized access.

You must not share access credentials or allow another person to impersonate you.

DEYPAY may require email verification, phone verification, additional authentication challenges or other reasonable security measures.

DEYPAY may temporarily restrict access, revoke sessions or require additional verification to protect accounts and customers.

DEYPAY will never require you to disclose your password through an unsolicited message.

## 6. Customer Identity Verification and KYC

DEYPAY is responsible for operating its customer onboarding, identity verification and KYC decision-making processes, subject to applicable laws and its regulatory responsibilities.

Where necessary, DEYPAY may use specialist verification technology or service providers acting within defined contractual roles.

Customers may be required to provide identification details, proof of address, identity documents or other verification information appropriate to the services requested.

DEYPAY may undertake sanctions screening, politically exposed person checks, fraud prevention, risk assessment and ongoing compliance monitoring.

A customer may register and access eligible nonfinancial functionality before KYC approval.

Financial functions remain restricted until the required identity, compliance, product, provider and legal conditions have been satisfied.

DEYPAY may reject or suspend a customer's eligibility where required by law or justified by documented security or compliance concerns.

## 7. Digital Assets and Stablecoin Services

Where approved and enabled, DEYPAY may support specified stablecoins, including USDT and USDC, on selected blockchain networks.

Only assets and networks expressly identified as supported in the application should be used.

Digital assets can involve significant risks, including loss of value, issuer and reserve risks, blockchain network failures, transaction delays, cybersecurity incidents, custody risks and changes in law or regulation.

A stablecoin's reference to a fiat currency does not guarantee redemption, liquidity or maintenance of its reference value in every circumstance.

DEYPAY does not guarantee investment returns, interest or appreciation of digital assets.

Customers must verify the displayed asset, network, destination address, fees and transaction instructions before authorizing a transaction.

Unsupported or incorrectly directed transactions may be irreversible or unrecoverable.

## 8. Custody and Banking Infrastructure

Where stablecoin custody services are enabled, customer digital assets will be held under the disclosed and legally approved custody arrangement.

DEYPAY may use regulated or otherwise appropriately authorized custody infrastructure providers, as required by applicable law.

The specific custody provider, customer asset rights, safeguarding model, relevant risks and responsibility for loss must be disclosed before customers are permitted to deposit or hold assets.

DEYPAY does not represent that a digital asset balance is a bank deposit or that it benefits from deposit insurance unless such protection is expressly applicable and verified.

Bank transfers, naira collections, naira payouts, liquidity conversion and settlement may be facilitated through appropriately approved partners.

A DEYPAY account is not automatically a bank account or a general-purpose stored-value naira wallet.

Any applicable provider-specific terms will be identified before the relevant service is used.

## 9. Payments, Transfers and Conversion

Where such features have been enabled, customers may submit eligible transaction instructions through DEYPAY.

A transaction may be subject to balance checks, customer authorization, sanctions and fraud screening, network confirmation, liquidity availability, provider acceptance and settlement checks.

The application must display material transaction terms, including available amounts, exchange rates, fees and recipient details, before final authorization.

Exchange rates may vary with market and liquidity conditions. A quoted rate applies only for its stated validity period and subject to its disclosed conditions.

Transaction requests may be rejected, delayed, placed under review or cancelled when required by law, security controls, provider conditions or system availability.

A transaction shown as submitted or processing is not necessarily final or settled.

## 10. Fees and Charges

Applicable fees will be disclosed before customers authorize a chargeable transaction.

Fees may include DEYPAY service fees, conversion spreads, third-party payment fees and blockchain network charges, where relevant.

DEYPAY will not represent an undisclosed charge as a confirmed customer fee.

Applicable taxes may arise from a customer's activities. Customers remain responsible for their own tax obligations, subject to applicable law.

## 11. Transaction Errors, Disputes and Complaints

Customers should report suspected errors or unauthorized activity promptly through DEYPAY's official support channels.

DEYPAY will review complaints, investigate relevant records, coordinate with infrastructure providers where necessary and communicate outcomes in accordance with applicable requirements.

The possibility of reversing, recalling or recovering a payment depends on its status, applicable law, network characteristics and relevant provider capabilities.

Nothing in these Terms removes customer rights or remedies that cannot lawfully be excluded.

The published complaints procedure will specify acknowledgement, escalation and response arrangements.

## 12. Prohibited Activities

You must not use DEYPAY to:

- Commit fraud, theft, impersonation or money laundering.
- Finance terrorism or other unlawful activities.
- Evade sanctions, identification or transaction controls.
- Misuse another person's identity, credentials or financial information.
- Interfere with DEYPAY systems or exploit security vulnerabilities.
- Submit deliberately misleading payment, recipient or identity information.
- Engage in activity prohibited by applicable law or expressly published platform rules.

DEYPAY may investigate and act upon suspected misuse, including applying restrictions or making legally required disclosures.

## 13. Availability and Service Changes

DEYPAY may introduce, modify, suspend or discontinue features subject to applicable contractual and regulatory obligations.

Service availability may be affected by maintenance, network conditions, infrastructure partners or regulatory developments.

We will provide applicable notice of material changes and will not use these Terms to avoid non-excludable legal obligations.

## 14. Intellectual Property

DEYPAY's software, product design, trademarks, documentation and proprietary technology are owned by Kenule Africa or their respective lawful owners, as applicable.

You receive a limited, revocable, non-transferable right to access DEYPAY for lawful personal use, subject to these Terms.

You must not misuse or commercially exploit DEYPAY intellectual property without authorization.

## 15. Privacy and Data Protection

DEYPAY processes personal data as described in the DEYPAY Privacy Notice and in accordance with applicable data-protection requirements.

Our authentication and KYC responsibilities are separate from the banking, liquidity and custody infrastructure provided by third parties.

Where disclosures to service providers are necessary, they will be governed by applicable lawful bases, transparency requirements and contractual protections.

## 16. Account Restrictions and Closure

You may request closure of your DEYPAY account through the applicable account management or support process.

We may restrict or close accounts where necessary for legal compliance, security, fraud prevention or material breach of these Terms.

Account closure does not automatically erase information that must be retained for applicable legal, regulatory or dispute-resolution purposes.

When financial services are enabled, closure procedures must also address outstanding transactions, customer entitlements and permitted return or transfer of assets.

## 17. Liability and Customer Protections

DEYPAY will exercise reasonable care and skill in providing its services and will comply with obligations imposed by applicable law.

DEYPAY is not responsible for losses caused solely by circumstances beyond its reasonable control where the law permits such a limitation.

Nothing in these Terms excludes liability for fraud, willful misconduct, unlawful treatment of customer assets, or any liability that applicable law prohibits us from excluding.

Any additional financial-service-specific liability provisions must be approved and disclosed before those services are activated.

## 18. Changes to These Terms

DEYPAY may update these Terms to reflect changes in the product, law, security requirements or operating arrangements.

Material updates will be communicated as required by law.

Where a new affirmative acceptance is required, DEYPAY will request it before continuing affected services.

Historical versions and applicable acceptance records will be preserved according to the approved retention policy.

## 19. Governing Law

These Terms are governed by the laws of the Federal Republic of Nigeria, subject to applicable mandatory consumer-protection rights.

Disputes may be referred to the appropriate Nigerian courts or other lawful dispute-resolution bodies. The final dispute-resolution clause is subject to legal approval.

## 20. Contact and Legal Information

**Product:** DEYPAY  
**Parent Organization:** Kenule Africa  
**Legal Operator:** [Verified registered entity]  
**Registration Number:** [Insert where applicable]  
**Registered Address:** [Insert]  
**Customer Support:** [Insert]  
**Legal Contact:** [Insert]

**Status:** Draft — not effective until formally approved and published.$legal$,'UTF8'));

INSERT INTO customers.registration_legal_releases(scope,terms_version,privacy_version,wording_version)
VALUES ('sandbox','1.0','1.0','1.0');
-- No current release is selected by migrating a database. Only the sandbox owner opts in.

-- Preserve earlier evidence as recorded. New evidence links to the exact immutable
-- bundle, including the third document that supplies the acknowledgement wording.
ALTER TABLE customers.registration_consents
    ADD UNIQUE (user_id,terms_version,privacy_version),
    ALTER COLUMN recorded_at SET DEFAULT clock_timestamp();
CREATE TABLE customers.registration_consent_evidence (
    event_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL UNIQUE,
    terms_version text NOT NULL, privacy_version text NOT NULL, release_id bigint NOT NULL,
    terms_accepted boolean NOT NULL CHECK (terms_accepted),
    privacy_acknowledged boolean NOT NULL CHECK (privacy_acknowledged),
    request_id uuid NOT NULL UNIQUE, channel text NOT NULL CHECK (channel='REGISTRATION_API'),
    FOREIGN KEY (user_id,terms_version,privacy_version) REFERENCES customers.registration_consents(user_id,terms_version,privacy_version),
    FOREIGN KEY (release_id,terms_version,privacy_version) REFERENCES customers.registration_legal_releases(id,terms_version,privacy_version)
);
CREATE TRIGGER immutable_consent_evidence BEFORE UPDATE OR DELETE OR TRUNCATE
    ON customers.registration_consent_evidence FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

DROP FUNCTION identity.register_customer(text,text,text,text,text,text,text,text,text,bigint);

CREATE FUNCTION identity.register_customer(
    p_email text, p_phone text, p_username text, p_password_hash text,
    p_first_name text, p_last_name text, p_country_code text,
    p_terms_version text, p_privacy_version text, p_accept_terms boolean, p_ack_privacy boolean
) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    new_user uuid;
    request_id uuid := gen_random_uuid();
    selected customers.registration_legal_releases%ROWTYPE;
BEGIN
    SELECT r.* INTO selected FROM customers.current_registration_release c
        JOIN customers.registration_legal_releases r ON r.id=c.release_id
        WHERE c.scope='sandbox' FOR SHARE OF c;
    IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE='DP503', MESSAGE='Registration unavailable'; END IF;
    IF p_accept_terms IS NOT TRUE OR p_ack_privacy IS NOT TRUE
       OR selected.terms_version IS DISTINCT FROM p_terms_version
       OR selected.privacy_version IS DISTINCT FROM p_privacy_version THEN
        RAISE EXCEPTION USING ERRCODE='DP422', MESSAGE='Invalid registration request';
    END IF;
    IF EXISTS (SELECT FROM identity.users WHERE phone_e164 = p_phone AND phone_verified_at IS NOT NULL) THEN
        RETURN;
    END IF;
    INSERT INTO identity.users (email, phone_e164, status)
        VALUES (lower(btrim(p_email)), p_phone, 'CONTACT_UNVERIFIED')
        ON CONFLICT (email) DO NOTHING RETURNING id INTO new_user;
    IF new_user IS NULL THEN RETURN; END IF;

    -- The Phase 2 key table, partial unique index and reservation FK serialize
    -- racing claims. The exception rolls back this entire registration.
    BEGIN
        INSERT INTO customers.usernames (user_id, username) VALUES (new_user, p_username);
    EXCEPTION WHEN unique_violation OR foreign_key_violation THEN
        RAISE EXCEPTION USING ERRCODE = 'DP001', MESSAGE = 'registration conflict';
    END;
    INSERT INTO customers.profiles (user_id, first_name, last_name, country_code)
        VALUES (new_user, p_first_name, p_last_name, p_country_code);
    INSERT INTO identity.password_credentials (user_id, password_hash) VALUES (new_user, p_password_hash);
    INSERT INTO customers.registration_consents (user_id, terms_version, privacy_version, accepted_at)
        VALUES (new_user, p_terms_version, p_privacy_version, statement_timestamp());
    INSERT INTO customers.registration_consent_evidence
        (user_id,terms_version,privacy_version,release_id,terms_accepted,privacy_acknowledged,request_id,channel)
        VALUES (new_user,p_terms_version,p_privacy_version,selected.id,p_accept_terms,p_ack_privacy,request_id,'REGISTRATION_API');
    INSERT INTO audit.audit_events (actor_type, actor_id, action, resource_type, resource_id, correlation_id)
        VALUES ('CUSTOMER', new_user, 'CUSTOMER_REGISTERED', 'CUSTOMER', new_user, request_id);
    INSERT INTO integrations.outbox_events (aggregate_type, aggregate_id, event_type, payload)
        VALUES ('CUSTOMER', new_user, 'EMAIL_VERIFICATION_REQUESTED', '{}'),
               ('CUSTOMER', new_user, 'PHONE_VERIFICATION_REQUESTED', '{}');
    RETURN;
EXCEPTION WHEN SQLSTATE 'DP001' THEN
    RETURN;
END;
$$;
REVOKE ALL ON FUNCTION identity.register_customer(text,text,text,text,text,text,text,text,text,boolean,boolean) FROM PUBLIC;
