# DEYPAY REGISTRATION CONSENT & ACCEPTANCE RECORD

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

**Status:** Engineering and compliance draft. The approved registration fields are a product decision, but final legal publication and security activation require their respective approvals.