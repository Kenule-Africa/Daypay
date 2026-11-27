# DEYPAY (DP)
# API SPECIFICATION AND CONTRACT DESIGN
## REST API Architecture, Authentication, Wallet Operations, Payment Orchestration, Provider Integration and Administrative Interfaces

**Document ID:** DP-API-004  
**Version:** 1.0  
**Status:** Proposed Engineering Baseline — Pending Implementation and Provider Validation  
**Product:** DEYPAY  
**Initial Market:** Nigeria  
**Backend:** Rust / Axum / Tokio / SQLx  
**Database:** PostgreSQL  
**API Style:** REST / JSON  
**API Version:** v1  
**Initial Customer Assets:** USDC and USDT  
**Settlement Currency:** NGN  
**Preferred Infrastructure Candidate:** Nuvion  
**Date:** 18 November 2026  
**Classification:** Confidential — Internal Engineering

**Related Documents**

- DP-001 — Functional Requirements Specification
- DP-002 — System Architecture Document
- DP-003 — Database Design Specification
- DP-005 — Financial Ledger and Accounting Design Specification
- DP-006 — Security Architecture and Threat Model

---

# DOCUMENT CONTROL

## 1. Purpose

This document defines the proposed application programming interface for the DEYPAY platform.

It establishes the contracts required for:

- Customer registration and authentication.
- Identity verification and KYC.
- Customer profiles and usernames.
- USDC and USDT wallets.
- Blockchain deposit addresses.
- Internal DEYPAY transfers.
- External stablecoin withdrawals.
- NGN funding.
- Stablecoin conversions.
- Nigerian bank withdrawals.
- Third-party Nigerian bank payments.
- Beneficiary management.
- Transaction history.
- Payment links and QR payments.
- Notifications.
- Customer support.
- Administrative operations.
- Provider integration.
- Financial reconciliation.

The specification is intended to guide backend implementation, frontend integration, automated testing, API documentation and future mobile application development.

It is a proposed API contract, not a claim that the endpoints already exist or that all provider capabilities have been confirmed.

## 2. Document Authority

The Functional Requirements Specification defines product behavior.

The System Architecture Document defines architectural responsibilities.

The Database Design Specification defines persistence structures.

The Financial Ledger and Accounting Design Specification defines financial posting rules.

This document defines how authorized clients and internal systems request and observe those behaviors.

If an API contract conflicts with financial integrity or compliance requirements, the affected contract must be corrected before implementation.

---

# TABLE OF CONTENTS

1. Executive Summary
2. API Architecture and Design Principles
3. System Boundaries and Ownership
4. API Environments and Versioning
5. Protocol and Data Standards
6. Authentication Architecture
7. Authorization and Access Control
8. API Security Controls
9. Standard Request and Response Contracts
10. Error Handling and Error Codes
11. Idempotency and Retry Contracts
12. Pagination, Filtering and Sorting
13. User Registration APIs
14. Authentication and Session APIs
15. Customer Profile APIs
16. Username and Recipient Discovery APIs
17. KYC and Compliance APIs
18. Wallet APIs
19. Deposit and Blockchain Network APIs
20. Internal Transfer APIs
21. External Stablecoin Withdrawal APIs
22. FX Quote and Conversion APIs
23. NGN On-Ramp APIs
24. Nigerian Bank Directory and Verification APIs
25. Beneficiary Management APIs
26. Own-Bank Withdrawal APIs
27. Pay Anyone APIs
28. Unified Transaction APIs
29. Payment Link and QR APIs
30. Notification APIs
31. Customer Support APIs
32. Administrative APIs
33. Treasury and Reconciliation APIs
34. Nuvion Integration Architecture
35. Other Provider Integration Contracts
36. Webhook Architecture
37. Background Jobs and Asynchronous Processing
38. Financial Transaction State Contracts
39. Validation and Business Rules
40. Privacy and Data Protection
41. Rate Limiting and Abuse Prevention
42. Observability and Auditability
43. OpenAPI Documentation
44. Rust Backend Implementation Structure
45. API Testing Strategy
46. Performance and Reliability Requirements
47. Deployment and Release Management
48. Implementation Sequence
49. Risks and Mitigations
50. Open Decisions and Dependencies
51. Acceptance Criteria
52. Final API Architecture Definition

---

# 1. EXECUTIVE SUMMARY

DEYPAY will expose a versioned REST API implemented in Rust using Axum.

The API will serve the responsive PWA and future mobile applications.

All customer financial operations must pass through DEYPAY-controlled backend services.

The backend is responsible for:

1. Authentication.
2. Authorization.
3. Customer identity.
4. KYC enforcement.
5. Transaction limits.
6. Fraud and compliance controls.
7. Financial quote validation.
8. Balance verification.
9. Ledger reservations.
10. Financial transaction orchestration.
11. Provider execution.
12. Settlement verification.
13. Transaction history.
14. Notifications.
15. Reconciliation.
16. Administrative oversight.

External providers must be accessed through internal integration adapters.

The frontend must not possess privileged provider API keys or independently execute financial transactions.

Financial operations must use idempotency, explicit state transitions and durable persistence.

The API must be designed for reliability, financial correctness and future extensibility.

---

# 2. API ARCHITECTURE AND DESIGN PRINCIPLES

## 2.1 REST Architecture

DEYPAY will use REST-style HTTP APIs with JSON payloads.

## 2.2 Versioning

Public application endpoints will use:

`/api/v1`

## 2.3 Resource-Oriented Design

Endpoints should represent identifiable resources.

Examples:

`/api/v1/wallets`

`/api/v1/transfers/internal`

`/api/v1/transactions`

## 2.4 Financial Correctness

API convenience must never override ledger integrity.

## 2.5 Backend Authority

The backend must independently validate every financial operation.

## 2.6 Provider Independence

Public API contracts must not expose provider-specific implementation details unless explicitly required for a supported user workflow.

## 2.7 Idempotency

Financial mutation requests must support durable idempotency.

## 2.8 Explicit States

Asynchronous operations must expose clear processing states.

## 2.9 Consistency

Responses must use consistent identifiers, amount representations, timestamps and error formats.

## 2.10 Documentation

All supported endpoints must be documented through OpenAPI.

---

# 3. SYSTEM BOUNDARIES AND OWNERSHIP

## 3.1 Frontend Responsibilities

The frontend is responsible for:

- Rendering interfaces.
- Collecting customer input.
- Displaying quotes.
- Displaying transaction states.
- Requesting authorized operations.
- Presenting notifications and receipts.

The frontend is not responsible for:

- Calculating authoritative balances.
- Executing privileged provider operations.
- Approving KYC.
- Finalizing financial settlement.
- Posting ledger entries.
- Bypassing transaction limits.

## 3.2 Backend Responsibilities

The DEYPAY backend owns all application business rules and financial orchestration.

## 3.3 Provider Responsibilities

Providers may execute approved infrastructure operations such as:

- Identity verification.
- Stablecoin custody.
- Blockchain transfers.
- Conversion.
- NGN collection.
- NGN payouts.

Their exact capabilities remain subject to written confirmation and integration testing.

## 3.4 Communication Model

```text
DEYPAY PWA / MOBILE APP
          |
          v
     DEYPAY API
          |
          v
 AUTHENTICATION / KYC
          |
          v
 PAYMENT ORCHESTRATION
          |
          +-------------------+
          |                   |
          v                   v
   FINANCIAL LEDGER     PROVIDER ADAPTERS
          |                   |
          v                   v
     POSTGRESQL        NUVION / OTHER
                          PROVIDERS
```

The backend is the controlled boundary between DEYPAY customers and external infrastructure.

---

# 4. API ENVIRONMENTS AND VERSIONING

## 4.1 Environments

Recommended environments:

- Local development.
- Automated testing.
- Staging.
- Production.

## 4.2 Base URL

Illustrative production URL:

`https://api.deypay.example/api/v1`

This is a placeholder, not an active domain.

## 4.3 Versioning Rules

Breaking contract changes require an approved versioning strategy.

Non-breaking additions may be introduced within v1.

## 4.4 Deprecation

Deprecated endpoints must have:

- Notice.
- Replacement guidance.
- Migration timeline.
- Defined removal date.

## 4.5 Environment Isolation

Staging and production must use separate:

- Credentials.
- Databases.
- Provider configurations.
- Webhook secrets.
- Financial test accounts.

---

# 5. PROTOCOL AND DATA STANDARDS

## 5.1 Transport

HTTPS is required for production.

## 5.2 Content Type

`application/json`

## 5.3 Character Encoding

UTF-8.

## 5.4 Identifiers

Public resource identifiers should use UUIDs or approved opaque identifiers.

## 5.5 Timestamps

Use ISO 8601 timestamps with explicit UTC indication.

Example:

`2026-11-18T12:30:00Z`

## 5.6 Financial Amounts

Financial amounts must be represented as decimal strings or exact integer accounting units.

JSON floating-point numbers must not be used for authoritative financial amounts.

Recommended public API representation:

```json
{
  "asset": "USDT",
  "amount": "25.50"
}
```

## 5.7 Currency Codes

Supported initial assets:

- USDC.
- USDT.
- NGN.

## 5.8 Blockchain Networks

Networks must use canonical backend-defined identifiers.

Example:

`ethereum`

`base`

`polygon`

`solana`

Actual availability must come from the backend capability registry.

## 5.9 Monetary Precision

The API must validate amount precision according to the relevant asset and network.

---

# 6. AUTHENTICATION ARCHITECTURE

## 6.1 Authentication Model

The Next.js PWA uses an opaque, server-side session with a random cookie identifier. The backend stores its hash, enforces configured idle and absolute lifetimes, and rejects expired or revoked sessions. Privileged administrators must also complete strong MFA.

## 6.2 Browser Security

Set Secure, HttpOnly and SameSite=Lax on the PWA session cookie. Login, privilege changes and authenticated refresh rotate the identifier and invalidate the prior one. Refresh requires an active session and cannot extend its absolute lifetime.

Cross-site request forgery protections must be applied to cookie-authenticated state-changing requests.

## 6.3 Mobile Security

Future mobile applications may use an approved token-based session model with platform-secure credential storage.

## 6.4 Password Security

Passwords must be securely hashed using an approved algorithm such as Argon2id.

## 6.5 Sensitive Actions

Financial operations may require additional verification.

Examples:

- New device.
- New withdrawal address.
- High-value transfer.
- Bank beneficiary change.
- Account recovery.

## 6.6 Session Revocation

Users must be able to revoke sessions.

Logout revokes the current session. Successful account recovery revokes all sessions and outstanding transaction challenges and proofs. Enforce the recovery withdrawal hold in 06-SECURITY-COMPLIANCE.md SEC-REC-005.

---

# 7. AUTHORIZATION AND ACCESS CONTROL

## 7.1 Authorization Layers

Authorization must consider:

1. Authentication status.
2. Customer account status.
3. KYC status.
4. Transaction limits.
5. Account restrictions.
6. Resource ownership.
7. Required financial permissions.
8. Risk controls.

## 7.2 Customer Authorization

Customers may access only resources they own or are explicitly permitted to access.

## 7.3 Administrative Authorization

Administrative APIs require separate role-based permissions.

## 7.4 Financial Authorization

Customer-initiated financial execution requires confirmation of the exact transaction (§20.6). High-risk execution also requires step-up verification. Check shared limits atomically at acceptance as specified in 03-DATABASE-DESIGN.md §10.6.

## 7.5 Object-Level Authorization

Every endpoint accepting a resource ID must validate the caller's access to that resource.

---

# 8. API SECURITY CONTROLS

Required controls include:

- TLS.
- Authentication.
- Authorization.
- Request validation.
- Rate limiting.
- Abuse detection.
- Secure cookies where applicable.
- CSRF protection.
- Sensitive response redaction.
- Audit logging.
- Provider credential isolation.
- Webhook signature verification.
- Financial idempotency.

Sensitive information must never be accepted as proof of authorization merely because it appears in a request body.

---

# 9. STANDARD REQUEST AND RESPONSE CONTRACTS

## 9.1 Successful Response

Recommended response format:

```json
{
  "data": {
    "id": "9d4a7c10-2a20-4c9e-a987-48b781aa0c55",
    "status": "completed"
  },
  "meta": {
    "request_id": "c73c8b0e-67e0-44c8-8ef7-8424c2d5a55f"
  }
}
```

## 9.2 Error Response

```json
{
  "error": {
    "code": "INSUFFICIENT_FUNDS",
    "message": "Insufficient available balance.",
    "details": {}
  },
  "meta": {
    "request_id": "c73c8b0e-67e0-44c8-8ef7-8424c2d5a55f"
  }
}
```

## 9.3 Response Metadata

Responses should include a request identifier.

## 9.4 Sensitive Data

Responses must not expose:

- Password hashes.
- Provider secrets.
- Internal risk scores.
- Private keys.
- Full sensitive identity documents.
- Unnecessary internal financial infrastructure details.

---

# 10. ERROR HANDLING AND ERROR CODES

## 10.1 HTTP Status Codes

| Status | Meaning |
|---|---|
| 200 | Successful request |
| 201 | Resource created |
| 202 | Accepted for asynchronous processing |
| 204 | Successful request with no response body |
| 400 | Invalid request |
| 401 | Authentication required |
| 403 | Operation forbidden |
| 404 | Resource not found |
| 409 | Conflict |
| 410 | Resource expired or unavailable |
| 422 | Business validation failure |
| 429 | Rate limit exceeded |
| 500 | Internal server error |
| 502 | Upstream provider failure |
| 503 | Service temporarily unavailable |
| 504 | Upstream timeout |

## 10.2 Standard Business Error Codes

- VALIDATION_ERROR.
- AUTHENTICATION_REQUIRED.
- INVALID_CREDENTIALS.
- SESSION_EXPIRED.
- ACCOUNT_RESTRICTED.
- KYC_REQUIRED.
- KYC_PENDING.
- KYC_REJECTED.
- INSUFFICIENT_FUNDS.
- INVALID_AMOUNT.
- UNSUPPORTED_ASSET.
- UNSUPPORTED_NETWORK.
- QUOTE_EXPIRED.
- QUOTE_UNAVAILABLE.
- BENEFICIARY_INVALID.
- BENEFICIARY_VERIFICATION_FAILED.
- TRANSACTION_LIMIT_EXCEEDED.
- DUPLICATE_REQUEST.
- IDEMPOTENCY_CONFLICT.
- TRANSACTION_NOT_FOUND.
- TRANSACTION_NOT_CANCELLABLE.
- PROVIDER_UNAVAILABLE.
- PROVIDER_OUTCOME_UNKNOWN.
- TRANSACTION_UNDER_REVIEW.
- INTERNAL_ERROR.

## 10.3 Error Safety

Provider timeouts must not automatically be represented as confirmed payment failures.

---

# 11. IDEMPOTENCY AND RETRY CONTRACTS

## 11.1 Idempotency Header

Customer financial execution endpoints require:

`Idempotency-Key`

## 11.2 Key Scope

Keys must be scoped to the authenticated customer and operation.

## 11.3 Repeated Request

Same key and same request:

Return the existing operation or stored response.

## 11.4 Conflicting Request

Same key with different request content:

Return HTTP 409.

## 11.5 Durable Storage

Idempotency must survive application restarts.

## 11.6 Provider Retry

External operations must not be blindly repeated when their execution outcome is unknown.

---

# 12. PAGINATION, FILTERING AND SORTING

## 12.1 Pagination

Cursor-based pagination is recommended for transaction history.

## 12.2 Example

`GET /api/v1/transactions?limit=20&cursor=...`

## 12.3 Filters

Supported filters may include:

- Transaction type.
- Asset.
- Status.
- Start date.
- End date.

## 12.4 Sorting

Transaction history should default to newest first.

## 12.5 Pagination Limits

The backend must enforce a maximum page size.

---

# 13. USER REGISTRATION APIS

## 13.1 Register Customer

**Endpoint**

`POST /api/v1/auth/register`

**Authentication**

Public.

**Request**

```json
{
  "first_name": "Ada",
  "last_name": "Okafor",
  "email": "ada@example.com",
  "phone": "+2348012345678",
  "country_code": "NG",
  "username": "adaokafor",
  "password": "A1!xxx",
  "terms_version": "1.0",
  "privacy_version": "1.0",
  "accept_terms": true,
  "acknowledge_privacy_notice": true
}
```

**Response**

HTTP 202 for a valid submission, replay or conflict with an existing identity or reserved username. These cases return the same body and headers.

```json
{"data": {}}
```

All eleven fields are required. The example uses the sandbox drafts. Require two independently selected booleans equal to true and the exact active versions. DEYPAY generates the acceptance time and correlation ID. Unknown fields, including a client timestamp, return 400. Invalid field values, stale versions or a declined acknowledgement return 422. Rate limiting returns 429. An inactive release, unavailable mandatory screening, or disabled registration returns 503. DEYPAY creates an unverified customer and queues verification. The acknowledgement contains no identity, credentials or session cookie.

### Registration legal documents

`GET /api/v1/legal/documents` returns `data.scope` and an ordered `data.documents` array for the current registration release. The identifiers are DEP-LEG-001 (Terms), DEP-LEG-002 (Privacy) and DEP-LEG-003 (checkbox wording). Each entry includes `document_id`, `title`, `version`, `status`, `sha256` and a version-specific `url`. Clients display the sandbox label when `status` is `sandbox_draft`. Present separate, initially unchecked Terms and Privacy controls with links to those revisions. Optional marketing is a separate choice outside this request.

`GET /api/v1/legal/documents/{document_id}/{version}` returns the referenced UTF-8 Markdown bytes, with `X-Deypay-Legal-Status` and `Cache-Control: no-store`. A digest covers exactly those bytes. Versions remain available within their release scope after a current release is replaced. Unknown revisions and the internal DEP-LEG-004 evidence specification return 404. Production lists only formally published releases. An empty list means there is no current release, and a registry outage returns 503.

## 13.2 Registration Rules

- Email must be normalized.
- The required phone must use E.164 format. Registration stores it with `phone_verified_at` NULL and queues only `EMAIL_VERIFICATION_REQUESTED`.
- Username must be available.
- Password must satisfy security policy.
- Duplicate verified identity rules must be enforced.
- Financial activity requires separate KYC approval and eligibility checks after contact verification.

## 13.3 Registration Atomicity

Create the internal identity at registration. Eligibility approval triggers creation of the asset profiles and asynchronous provider provisioning under architecture §13.2. Publish deposit addresses only after verification.

---

# 14. AUTHENTICATION AND SESSION APIS

## 14.1 Login

`POST /api/v1/auth/login`

Request:

```json
{
  "identifier": "customer@example.com",
  "password": "user-supplied-secret"
}
```

Response:

Authenticated session or additional verification challenge.

## 14.2 Logout

`POST /api/v1/auth/logout`

## 14.3 Refresh Session

`POST /api/v1/auth/refresh`

## 14.4 Verify Email

`POST /api/v1/auth/verify-email`

Email is the sole MVP contact-verification requirement. Consuming a valid proof atomically records `email_verified_at` and advances an account in its initial contact state to `CONTACT_VERIFIED`. DEYPAY evaluates KYC and financial eligibility separately.

Submit `{"token":"<selector>.<secret>"}` as JSON. The email points to the configured application origin at `/verify-email#<token>`. Opening the link displays a confirmation button. Selecting it submits the token in the request body. The browser removes the fragment from its history entry. A successful submission returns HTTP 200 with `{"data":{}}`. Expired, incorrect, exhausted, superseded and consumed tokens return the same 422 response. A malformed JSON contract returns 400, admission limits return 429 and unavailable verification returns 503. Responses use `Cache-Control: no-store`.

## 14.5 Verify Phone

`POST /api/v1/auth/verify-phone` is reserved for post-MVP implementation. MVP clients collect the phone number at registration and prompt only for email verification.

## 14.6 Resend Verification

`POST /api/v1/auth/resend-verification`

For email verification submit `{"email":"customer@example.com"}`. Registration initiates the first email. A valid resend request returns HTTP 202 with `{"data":{}}` for known, unknown, already-verified and per-address throttled recipients. An eligible resend supersedes the previous link. Each endpoint also has a deployment-wide request limit.

## 14.7 Forgot Password

`POST /api/v1/auth/forgot-password`

Submit `{"email":"customer@example.com"}` as JSON with the configured browser `Origin`. The response is HTTP 202 with `{"data":{}}` for every valid address: known, unknown, unverified, ineligible, within the per-account cooldown or hourly cap, or over the address admission budget. The repository owner approved the recovery policy on 2026-11-25. A reset email goes only to a registered, already verified address whose account is in one of the active authentication release's allowed states. The email links to `/reset-password#<selector>.<secret>` on the configured application origin. The link is single use and expires after 30 minutes by default. A newer request supersedes older links. Each account receives at most one reset email per cooldown period (60 seconds by default) and a capped number per hour (5 by default). A malformed address returns 422 `VALIDATION_ERROR`. Unknown fields, a missing field or a non-JSON body return 400. A missing or foreign `Origin` returns 403. Admission limits return 429 and unavailable recovery returns 503. Responses use `Cache-Control: no-store`. The ingress routes this endpoint to the isolated authentication verifier; the general API returns 503.

## 14.8 Reset Password

`POST /api/v1/auth/reset-password`

Submit `{"token":"<selector>.<secret>","password":"<new password>"}` as JSON with the configured browser `Origin`. The reset page at `/reset-password` removes the token from the address bar and submits it in the body. The new password must satisfy the registration password policy and must not appear in the compromised-password dataset; it is stored only as an Argon2id hash. On success the response is HTTP 200 with `{"data":{}}` and an expired session cookie. Every session of the account is revoked, including the current one, so the customer signs in again with the new password (security SEC-REC-004). The reset is audited.

A malformed, unknown, expired, superseded, already used or exhausted link returns 422 `VALIDATION_ERROR` with `details.reason` `invalid_link`. The fifth wrong secret (by default) revokes the link. A rejected password returns 422 with `details.reason` `password_rejected` and does not spend the link. Unknown fields, missing fields or a non-JSON body return 400. A missing or foreign `Origin` returns 403. Admission limits return 429 and unavailable recovery returns 503. Responses use `Cache-Control: no-store`. The ingress routes this endpoint to the isolated authentication verifier; the general API returns 503.

## 14.9 List Sessions

`GET /api/v1/auth/sessions`

## 14.10 Revoke Session

`DELETE /api/v1/auth/sessions/{session_id}`

## 14.11 Authentication Requirements

- Login attempts must be rate-limited.
- Password reset tokens must be short-lived and single-use.
- Session revocation must be enforced.
- Sensitive authentication events must be audited.

---

# 15. CUSTOMER PROFILE APIS

Every route in this section requires an active customer session and serves only that customer. A customer identifier in a path, query, header or body is never accepted. Missing, expired, logged-out and revoked sessions return 401 `AUTHENTICATION_REQUIRED`. State-changing requests must carry the configured browser `Origin`, or they return 403. JSON responses use `Cache-Control: no-store`. A route returns 503 `SERVICE_UNAVAILABLE` when customer authentication or profile storage is unavailable. The repository owner approved the editable fields, preference defaults, marketing-consent handling and profile-image contract below on 2026-11-25.

## 15.1 Get Current Profile

`GET /api/v1/me`

```json
{
  "data": {
    "id": "9d4a7c10-2a20-4c9e-a987-48b781aa0c55",
    "username": "adaokafor",
    "first_name": "Ada",
    "last_name": "Okafor",
    "email": "ada@example.com",
    "email_verified": true,
    "phone": "+2348012345678",
    "phone_verified": false,
    "country_code": "NG",
    "account_status": "contact_verified",
    "names_editable": true,
    "profile_image": null,
    "created_at": "2026-11-25T10:00:00Z",
    "updated_at": "2026-11-25T10:00:00Z"
  }
}
```

`profile_image` is `null` or the object described in §15.7. The response contains no credential, session, audit, risk or KYC data.

## 15.2 Update Profile

`PATCH /api/v1/me`

Send a JSON object with `first_name`, `last_name` or both. Each value is trimmed and must be 1–100 characters without control, invisible formatting, line-separator or non-space whitespace characters. Names can change while the account status is `contact_verified` or `kyc_required` and no KYC case has moved past draft; afterwards `names_editable` is `false`. A successful update returns the §15.1 profile.

Any other field, including the identifier, email, phone, username, country, account status, KYC or password, returns 400 `VALIDATION_ERROR`. So do `null`, non-string values, an empty object and a non-JSON body. Invalid names return 422 `VALIDATION_ERROR` with `details.fields` listing them. Locked names return 422 `VALIDATION_ERROR` with `details.reason` `names_locked`. Each update is atomic, and concurrent updates never mix fields from two requests. The audit event records which fields changed, not their values.

## 15.3 Get Preferences

`GET /api/v1/me/preferences`

```json
{
  "data": {
    "phone_discoverable": false,
    "email_notifications": true,
    "push_notifications": false,
    "marketing_consent": false,
    "updated_at": null
  }
}
```

These are the defaults before a customer changes anything; `updated_at` is then `null`. The notification flags cover optional notifications only. Security and service messages do not depend on them.

## 15.4 Update Preferences

`PATCH /api/v1/me/preferences`

Send a JSON object with one or more of the four boolean fields. Omitted fields stay unchanged, so concurrent updates to different fields are all kept. Other fields, `null`, non-boolean values and an empty object return 400. Setting `phone_discoverable` to `true` without a verified phone returns 422 `VALIDATION_ERROR` with `details.reason` `phone_unverified` (§16.5), and nothing is changed. `marketing_consent` can be granted or withdrawn; its audit event cites the DEP-LEG-003 consent wording the customer registered under. A successful update returns the §15.3 response.

## 15.5 Get Account Status

`GET /api/v1/me/status`

```json
{"data": {"account_status": "contact_verified"}}
```

The value is the FRS §8.4 account status in lowercase. It is the account lifecycle status. It is neither a KYC decision nor financial authorization, and no profile route can change it.

## 15.6 Profile Requirements

The customer must not be able to modify authoritative KYC fields without the approved verification process.

## 15.7 Profile Image

The customer's own profile image is private. Another customer cannot read, replace or remove it.

`PUT /api/v1/me/profile-image` uploads or replaces the image. Send the raw bytes with `Content-Type` `image/jpeg`, `image/png` or `image/webp`. The bytes must be that format, at most 5 MB (5,242,880 bytes) and at most 4096 pixels on each side. DEYPAY decodes the image within a fixed memory budget, applies its EXIF orientation, flattens transparency onto white, reduces the longest side to at most 512 pixels without enlarging it, and stores a new JPEG of at most 150 KB without any EXIF, GPS, ICC or other source metadata. Only that JPEG is ever stored or served. The response is:

```json
{
  "data": {
    "profile_image": {
      "version": "3f1d2b8e-7c55-4d1e-9a51-1b4c1f0d2e77",
      "content_type": "image/jpeg",
      "width": 512,
      "height": 384,
      "updated_at": "2026-11-25T10:05:00Z"
    }
  }
}
```

A missing or other type, or content that is not the declared type, returns 415. A larger body returns 413. An image that cannot be decoded, exceeds the dimension or memory limits, or cannot meet the size limit returns 422. Errors use `VALIDATION_ERROR` with `details.reason` `unsupported_type`, `too_large`, `undecodable`, `too_many_pixels` or `too_complex`. A body not received within 30 seconds returns 400 `VALIDATION_ERROR` with `details.reason` `incomplete_upload`. Each instance processes a bounded number of uploads at once; when none can start within 5 seconds, the request returns 503 `SERVICE_UNAVAILABLE` with `details.reason` `busy` and can be retried. Each replacement is atomic: a failure leaves the previous image in place, and no superseded image remains.

`GET /api/v1/me/profile-image` returns the JPEG with `ETag` set to the quoted version, `Cache-Control: private, no-cache`, `Vary: Cookie`, `X-Content-Type-Options: nosniff` and `Cross-Origin-Resource-Policy: same-site`. A matching `If-None-Match` returns 304. Without an image it returns 404 `NOT_FOUND`.

`DELETE /api/v1/me/profile-image` removes the image and returns 204, also when no image exists.

Images are stored in PostgreSQL, separate from KYC documents, behind a storage interface that an object-storage adapter can replace once the object-storage service is chosen (architecture §46.4).

---

# 16. USERNAME AND RECIPIENT DISCOVERY APIS

## 16.1 Check Username Availability

`GET /api/v1/usernames/availability?username=exampleuser`

The check is public and advisory; the repository owner approved this policy on 2026-11-25. A username in the §16.7 format returns HTTP 200:

```json
{"data": {"available": true}}
```

`available` is `false` when the username, ignoring letter case and the optional `@`, is reserved or belongs to an active account. The two cases are indistinguishable, and no information about an account is returned. Registration remains the authority, so a later registration can still find the username taken. A username outside the §16.7 format returns 422 `VALIDATION_ERROR` with `details.fields` `["username"]`. A missing, repeated or additional query parameter returns 400. Per-deployment and per-username admission limits return 429, and an unavailable check returns 503. Responses use `Cache-Control: no-store`.

## 16.2 Resolve Recipient

`GET /api/v1/recipients/resolve?username=exampleuser`

## 16.3 Response

```json
{
  "data": {
    "recipient_id": "2c5f2e63-5ec1-4c1f-8a39-507ce66bde31",
    "username": "exampleuser",
    "display_name": "Example User",
    "can_receive": true
  }
}
```

## 16.4 Privacy Requirements

Recipient resolution must not expose:

- Email address.
- Full phone number.
- KYC documents.
- Wallet balance.
- Internal compliance details.

## 16.5 Phone Discovery

Phone-based recipient discovery requires a verified phone, the recipient's discoverability preference and anti-enumeration controls. A phone collected at MVP signup remains ineligible while unverified.

## 16.6 Username Changes

Username changes must follow a controlled policy.

Historical transactions must reference immutable user IDs.

## 16.7 Username Format

A username contains only ASCII letters, digits, underscore and period. It is 3–30 characters long and does not start or end with a period or contain two consecutive periods. A leading `@` is optional and is not part of the username. Uniqueness and reserved-name checks ignore letter case. Registration returns 422 for any other format, and the database refuses new active usernames that do not comply. The repository owner approved this format on 2026-11-25.

---

# 17. KYC AND COMPLIANCE APIS

## 17.1 Get KYC Status

`GET /api/v1/kyc/status`

## 17.2 Start KYC

`POST /api/v1/kyc/start`

## 17.3 Submit KYC Details

`POST /api/v1/kyc/details`

## 17.4 Request Document Upload

`POST /api/v1/kyc/documents/upload-request`

## 17.5 Complete KYC Submission

`POST /api/v1/kyc/submit`

## 17.6 KYC Response

```json
{
  "data": {
    "kyc_case_id": "d6c4e289-3fd0-4b1c-aec8-47ab5d0c6e84",
    "status": "pending_provider",
    "financial_access_enabled": false
  }
}
```

## 17.7 KYC Status Values

- not_started.
- draft.
- submitted.
- pending_provider.
- manual_review.
- approved.
- rejected.
- expired.

## 17.8 KYC Enforcement

Financial endpoints must verify customer eligibility on the backend.

Frontend display restrictions alone are insufficient.

---

# 18. WALLET APIS

## 18.1 List Wallets

`GET /api/v1/wallets`

## 18.2 Get Wallet

`GET /api/v1/wallets/{wallet_id}`

## 18.3 Get Wallet Balance

`GET /api/v1/wallets/{wallet_id}/balance`

## 18.4 Example Balance Response

```json
{
  "data": {
    "wallet_id": "42e0fca0-5942-4ef0-974e-ef84594a35ef",
    "asset": "USDT",
    "available": "125.50",
    "reserved": "20.00",
    "total": "145.50"
  }
}
```

## 18.5 Balance Source

Balances must originate from the DEYPAY financial ledger.

Show pending deposits separately. The recognized total in §18.4 equals available plus reserved; pending amounts cannot be spent.

## 18.6 Wallet Status

Wallets may be:

- active.
- restricted.
- provisioning.
- suspended.
- closed.

## 18.7 Balance Security

The API must not accept a customer-supplied balance value as authoritative.

## 18.8 Implemented Contract (IMP-P6-006/009)

**Access and errors**
- Every route in §§18–19 requires an active customer session and serves only that customer; responses use `Cache-Control: no-store`.
- An unknown wallet, another customer's wallet and a malformed identifier all return the same 404 `NOT_FOUND`.
- When the wallet database or ledger is unavailable, the response is 503 `SERVICE_UNAVAILABLE`. No balance is substituted.

**Wallet creation**
- `GET /api/v1/wallets` first creates any missing USDC and USDT wallet, together with its zero-balance ledger accounts. This applies to customers who can hold a session.
- Creation is atomic and idempotent, and there is no customer NGN wallet.

**Wallet object**
- Fields: `id`, `asset`, `asset_name`, `status` (§18.6), `balance`, `restrictions`, `capabilities`, `created_at` and `updated_at`.
- `balance` holds `available`, `reserved` and `total` as exact decimal strings with the asset's accounting precision, or is null while the wallet is being set up.
- `restrictions` lists codes from: `wallet_provisioning`, `wallet_restricted`, `wallet_suspended`, `wallet_closed`, `account_restricted`, `kyc_required`, `asset_unavailable`, `asset_on_hold`, `financial_activity_not_enabled`.
- `capabilities` holds the booleans `deposit` and `send`.

**Wallet status**
- An active wallet is reported `restricted` while its account is restricted or its asset is suspended or on hold.
- `restricted` blocks outbound movement; `suspended` blocks all activity; `closed` is final and stays in history.
- Only an authenticated operator holding `WALLET_STATUS_CONTROL` changes a stored status. There is no customer endpoint for it.

**Other wallet routes**
- `GET /api/v1/wallets/{wallet_id}` adds `networks`: the enabled networks for the wallet's asset. Each includes its token contract and decimals, `deposit.available`, `deposit.address_status` (`active`, `pending`, `not_requested` or `unavailable`), the verified `deposit.address` only when active, and `withdrawal.available`.
- `GET /api/v1/wallets/{wallet_id}/balance` returns `wallet_id`, `asset`, `status`, `available`, `reserved` and `total`.
- Pending deposits are not part of any balance. They will be reported separately once deposit detection (Phase 9) exists.

---

# 19. DEPOSIT AND BLOCKCHAIN NETWORK APIS

## 19.1 List Supported Networks

`GET /api/v1/networks`

## 19.2 List Supported Asset Networks

`GET /api/v1/assets/{asset}/networks`

## 19.3 Get Deposit Address

`GET /api/v1/wallets/{wallet_id}/deposit-addresses?network=base`

## 19.4 Provision Deposit Address

`POST /api/v1/wallets/{wallet_id}/deposit-addresses`

Request:

```json
{
  "network": "base"
}
```

## 19.5 Response

```json
{
  "data": {
    "asset": "USDC",
    "network": "base",
    "address": "0x1234567890abcdef1234567890abcdef12345678",
    "memo": null,
    "status": "active"
  }
}
```

The address is illustrative.

### Implemented behaviour (IMP-P6-007/008)

**Supported networks**
- `GET /api/v1/networks` lists only enabled networks, each with its enabled asset contracts.
- `GET /api/v1/assets/{asset}/networks` does the same for one asset. Any asset other than USDC or USDT returns 422 `UNSUPPORTED_ASSET`.
- Nothing is presented as supported until the database administrator enables the network and a verified token contract.

**Reading a deposit address**
- `GET .../deposit-addresses` takes exactly one `network` query parameter and returns `address` and `memo` only for a verified, active address the wallet may receive on. Otherwise they are null, and `status` says why.

**Requesting a deposit address**
- `POST .../deposit-addresses` accepts exactly `{"network": "<code>"}` and needs the browser `Origin`.
- It returns:
  - the active address (200);
  - or a pending provisioning request (202), reusing an open one;
  - or one of these errors:
    - 422 `UNSUPPORTED_NETWORK`;
    - 403 `ACCOUNT_RESTRICTED` or `KYC_REQUIRED`;
    - 503 `SERVICE_UNAVAILABLE` with `details.reason` `deposits_not_enabled` (the asset's crediting is not activated);
    - 503 `PROVIDER_UNAVAILABLE` (no approved provider capability).
- The request never contacts a provider. A provider-integration worker submits it once, resolves an unknown outcome only by lookup with the same provider, and shows an assigned address only after verifying it.

## 19.6 Deposit History

`GET /api/v1/deposits`

## 19.7 Deposit Details

`GET /api/v1/deposits/{deposit_id}`

## 19.8 Deposit Rules

- Only supported networks may be used.
- Deposit address ownership must be verified.
- Deposits must satisfy required confirmations.
- Duplicate blockchain events must not create duplicate credits.
- Unsupported deposits require an exception process.

---

# 20. INTERNAL TRANSFER APIS

## 20.1 Create Transfer

`POST /api/v1/transfers/internal`

## 20.2 Request

```json
{
  "recipient_id": "2c5f2e63-5ec1-4c1f-8a39-507ce66bde31",
  "authorization_token": "short-lived-authorization-proof",
  "asset": "USDT",
  "amount": "25.00",
  "description": "Payment"
}
```

Required header:

`Idempotency-Key`

## 20.3 Response

```json
{
  "data": {
    "transaction_id": "3f7cf7a9-1f2d-4d6b-a830-8cf41a1e9ad4",
    "type": "internal_transfer",
    "status": "completed",
    "asset": "USDT",
    "amount": "25.00"
  }
}
```

## 20.4 Transfer Rules

- Sender must be authenticated.
- Sender must satisfy KYC requirements.
- Recipient must be eligible.
- Sender and recipient must differ.
- Available balance must be sufficient.
- Transfer must satisfy limits.
- Ledger debit and credit must be atomic.
- Duplicate requests must not duplicate transfers.

## 20.5 Fees

The initial internal transfer fee is configurable.

The proposed MVP default is zero, subject to commercial approval.

## 20.6 Authorization

1. Resolve the username to the immutable recipient ID (§16), or obtain a server quote bound to the verified destination. POST /api/v1/transaction-authorizations with the operation type and quote ID or the immutable transfer terms. The authenticated server returns a challenge ID, normalized display terms, expiry and required verification method.
2. Display those terms. POST /api/v1/transaction-authorizations/{challenge_id}/confirm with explicit customer confirmation and the required step-up response, if applicable. Verify the same session, terms and expiry, complete the challenge once and return a short-lived, single-use authorization_token.
3. Submit the proof with the financial request and Idempotency-Key. For the authenticated customer and operation, return the stored acceptance on identical replay; conflicting content returns HTTP 409. For a new request, verify the bound details from SEC-TXN-002, reject expired, used or mismatched proofs, and consume the proof atomically with acceptance. This contract also applies to withdrawals, FX, funding orders and bank payments.

---

# 21. EXTERNAL STABLECOIN WITHDRAWAL APIS

## 21.1 Get Withdrawal Quote

`POST /api/v1/withdrawals/quotes`

Request:

```json
{
  "asset": "USDT",
  "network": "ethereum",
  "destination_address": "0x1234567890abcdef1234567890abcdef12345678",
  "amount": "50.00"
}
```

## 21.2 Quote Response

```json
{
  "data": {
    "quote_id": "a52d0d0e-711c-45bd-a540-43d820d410db",
    "asset": "USDT",
    "amount": "50.00",
    "fee": "2.00",
    "total_debit": "52.00",
    "expires_at": "2026-11-18T12:35:00Z"
  }
}
```

Values are illustrative.

## 21.3 Execute Withdrawal

`POST /api/v1/withdrawals`

Request:

```json
{
  "quote_id": "a52d0d0e-711c-45bd-a540-43d820d410db",
  "authorization_token": "short-lived-authorization-proof"
}
```

Required header:

`Idempotency-Key`

## 21.4 Withdrawal Response

HTTP 202.

```json
{
  "data": {
    "transaction_id": "65dfbb35-9401-4b24-99f2-f3c285a20c14",
    "status": "processing"
  }
}
```

## 21.5 Withdrawal Requirements

For §21.2, reserve the full 52.00 USDT total debit once on acceptance. Consume the reserve on verified settlement or release the eligible amount on definitive non-execution as specified in 05-LEDGER-ACCOUNTING.md §15.

- Quote must be valid.
- Destination must be validated.
- Customer must authorize the operation.
- Funds must be reserved.
- Provider execution must be idempotent where supported.
- Unknown provider outcomes must remain reconcilable.
- Settlement status must be updated from verified evidence.

---

# 22. FX QUOTE AND CONVERSION APIS

## 22.1 Create FX Quote

`POST /api/v1/fx/quotes`

Request:

```json
{
  "source_asset": "USDC",
  "destination_asset": "USDT",
  "source_amount": "100.00"
}
```

## 22.2 Quote Response

```json
{
  "data": {
    "quote_id": "bd90b7e2-b2b9-438e-a1cf-bc87912bb0e9",
    "source_asset": "USDC",
    "destination_asset": "USDT",
    "source_amount": "100.00",
    "destination_amount": "99.50",
    "rate": "0.995",
    "fee": "0.00",
    "expires_at": "2026-11-18T12:35:00Z"
  }
}
```

## 22.3 Execute Conversion

`POST /api/v1/fx/conversions`

Request:

```json
{
  "quote_id": "bd90b7e2-b2b9-438e-a1cf-bc87912bb0e9",
  "authorization_token": "short-lived-authorization-proof"
}
```

## 22.4 Conversion Requirements

- Quote must be valid.
- Customer must have sufficient balance.
- Conversion must use the accepted quote.
- Settlement must be tracked.
- Accounting must balance independently by asset.
- Fees and provider execution must be reconciled.

---

# 23. NGN ON-RAMP APIS

## 23.1 Create Funding Quote

`POST /api/v1/onramp/quotes`

Request:

```json
{
  "source_currency": "NGN",
  "destination_asset": "USDT",
  "source_amount": "150000.00"
}
```

## 23.2 Quote Response

```json
{
  "data": {
    "quote_id": "d54ec1c5-faf1-45e7-b7c0-9b4d3bb42e9c",
    "source_currency": "NGN",
    "destination_asset": "USDT",
    "source_amount": "150000.00",
    "destination_amount": "100.00",
    "expires_at": "2026-11-18T12:35:00Z"
  }
}
```

Illustrative values only.

## 23.3 Create Funding Order

`POST /api/v1/onramp/orders`

Accept the customer’s quote_id, authorization_token and Idempotency-Key. Return HTTP 201 with order_id, transaction_id, expected NGN amount, destination asset, verified funding instructions, expiry and status. Enforce quote ownership and one execution. Track collection and delivery separately under database §15.3.

## 23.4 Get Funding Order

`GET /api/v1/onramp/orders/{order_id}`

Only the owning customer may retrieve the order, its instructions, status and credited transaction reference. Expired or mismatched collections enter investigation before any credit.

## 23.5 On-Ramp Requirements

- Customer must satisfy KYC requirements.
- Funding instructions must originate from an approved provider.
- Payment confirmation must be verified.
- Stablecoin delivery must be confirmed.
- Customer credit must be idempotent.
- NGN must not automatically become a customer stored-value balance.

## 23.6 Provider Dependency

Production enablement requires confirmed NGN collection and stablecoin delivery capability.

---

# 24. NIGERIAN BANK DIRECTORY AND VERIFICATION APIS

## 24.1 List Banks

`GET /api/v1/banks?country=NG`

## 24.2 Verify Bank Account

`POST /api/v1/banks/resolve-account`

Request:

```json
{
  "bank_code": "BANK_CODE",
  "account_number": "0123456789"
}
```

## 24.3 Response

```json
{
  "data": {
    "bank_code": "BANK_CODE",
    "account_number_masked": "******6789",
    "account_name": "Example Business",
    "verification_status": "verified"
  }
}
```

## 24.4 Requirements

- Bank directory must come from an approved source.
- Account-name verification must use trusted infrastructure.
- Responses must minimize sensitive data.
- Requests must be rate-limited.
- Verification results must not be treated as permanent.

---

# 25. BENEFICIARY MANAGEMENT APIS

## 25.1 List Beneficiaries

`GET /api/v1/beneficiaries`

## 25.2 Create Beneficiary

`POST /api/v1/beneficiaries`

Accept a type and private display_label. For an internal recipient, store the resolved recipient_id. For a bank recipient, store the verified bank destination under the authenticated owner. Return the beneficiary ID and masked display data.

## 25.3 Get Beneficiary

`GET /api/v1/beneficiaries/{beneficiary_id}`

`PATCH /api/v1/beneficiaries/{beneficiary_id}` accepts display_label only and returns the renamed beneficiary. Enforce owner access for both types.

## 25.4 Delete Beneficiary

`DELETE /api/v1/beneficiaries/{beneficiary_id}`

## 25.5 Beneficiary Rules

- Customer must own the beneficiary.
- Bank account information must be protected.
- Beneficiary creation may require verification.
- High-risk beneficiary changes may require step-up authentication.

---

# 26. OWN-BANK WITHDRAWAL APIS

## 26.1 Create Withdrawal Quote

`POST /api/v1/offramp/quotes`

Request:

```json
{
  "source_asset": "USDT",
  "destination_currency": "NGN",
  "beneficiary_id": "7a7f91ad-3f3c-41a6-84f4-4c0c1c31d9c4",
  "receive_amount": "30000.00"
}
```

## 26.2 Execute Withdrawal

`POST /api/v1/offramp/withdrawals`

Accept quote_id, authorization_token and Idempotency-Key. Return HTTP 202 with withdrawal_id, transaction_id and public status. Verify ownership of the actual destination bank account under the approved route as well as ownership of the saved record.

## 26.3 Get Withdrawal

`GET /api/v1/offramp/withdrawals/{withdrawal_id}`

## 26.4 Requirements

- Beneficiary must belong to the customer.
- Bank ownership must be verified according to policy.
- Quote must be accepted.
- Stablecoins must be reserved.
- Conversion and payout must be tracked separately.
- Payout completion must be verified.

---

# 27. PAY ANYONE APIS

## 27.1 Purpose

Pay Anyone enables DEYPAY customers to send NGN to supported Nigerian bank accounts using stablecoin balances.

Recipients do not need DEYPAY accounts.

## 27.2 Create Bank Payment Quote

`POST /api/v1/payments/bank/quotes`

Request:

```json
{
  "funding_asset": "USDT",
  "bank_code": "BANK_CODE",
  "account_number": "0123456789",
  "receive_amount": "30000.00"
}
```

## 27.3 Quote Response

```json
{
  "data": {
    "quote_id": "c965f1bd-f95a-45b4-9819-5d87a929d69e",
    "recipient_name": "Example Grocery Store",
    "recipient_bank": "Example Bank",
    "receive_currency": "NGN",
    "receive_amount": "30000.00",
    "funding_asset": "USDT",
    "funding_amount": "20.00",
    "fee": "0.00",
    "total_debit": "20.00",
    "expires_at": "2026-11-18T12:35:00Z"
  }
}
```

All values are illustrative.

For bank quotes in §§26–27, accept exactly one amount: receive_amount in NGN or source_amount in the selected funding asset. The server calculates the other side, applicable fees and total debit. Display the verified destination and quote before authorization.

## 27.4 Execute Bank Payment

`POST /api/v1/payments/bank`

Request:

```json
{
  "quote_id": "c965f1bd-f95a-45b4-9819-5d87a929d69e",
  "authorization_token": "short-lived-authorization-proof"
}
```

Required header:

`Idempotency-Key`

## 27.5 Response

HTTP 202.

```json
{
  "data": {
    "transaction_id": "de9fdac9-1c85-45fd-9e2d-0bd16bdc9d72",
    "status": "processing",
    "receive_amount": "30000.00",
    "receive_currency": "NGN"
  }
}
```

## 27.6 Pay Anyone Requirements

- Customer must be KYC-approved.
- Recipient account must be verified.
- Quote must be valid.
- Funding asset must be supported.
- Customer must authorize.
- Funds must be reserved.
- Conversion must be tracked.
- Bank payout must be executed through an approved provider.
- Completion must be verified.
- Unknown outcomes must remain reconcilable.
- Duplicate execution must be prevented.

## 27.7 Regulatory Dependency

Third-party bank payments must not be enabled in production until the applicable legal permissions, partner responsibilities and payout arrangements are confirmed.

---

# 28. UNIFIED TRANSACTION APIS

## 28.1 List Transactions

`GET /api/v1/transactions`

## 28.2 Get Transaction

`GET /api/v1/transactions/{transaction_id}`

## 28.3 Get Receipt

`GET /api/v1/transactions/{transaction_id}/receipt`

## 28.4 Transaction Response

```json
{
  "data": {
    "id": "de9fdac9-1c85-45fd-9e2d-0bd16bdc9d72",
    "reference": "DP-EXAMPLE-000001",
    "type": "bank_payment",
    "status": "completed",
    "source_asset": "USDT",
    "source_amount": "20.00",
    "destination_asset": "NGN",
    "destination_amount": "30000.00",
    "created_at": "2026-11-18T12:30:00Z",
    "completed_at": "2026-11-18T12:31:00Z"
  }
}
```

## 28.5 Transaction History Requirements

- Results must be customer-scoped.

- Include each internal transfer for its sender and recipient under the same transaction ID. Derive debit or credit direction for the caller. Apply the same participant check to detail and receipt queries, and deny unrelated callers.
- Pagination must be supported.
- Sensitive beneficiary data must be masked.
- Historical transaction records must remain consistent with ledger evidence.

---

# 29. PAYMENT LINK AND QR APIS

## 29.1 Create Payment Link

`POST /api/v1/payment-links`

## 29.2 List Payment Links

`GET /api/v1/payment-links`

## 29.3 Get Payment Link

`GET /api/v1/payment-links/{payment_link_id}`

## 29.4 Resolve Public Payment Link

`GET /api/v1/public/payment-links/{token}`

## 29.5 QR Payment

QR codes may encode an approved DEYPAY payment link or structured payment request.

## 29.6 Requirements

- Tokens must be unpredictable.
- Links may expire.
- Payment authorization remains mandatory.
- Public resolution must not expose private customer information.

---

# 30. NOTIFICATION APIS

## 30.1 List Notifications

`GET /api/v1/notifications`

## 30.2 Mark Notification Read

`POST /api/v1/notifications/{notification_id}/read`

## 30.3 Notification Preferences

`GET /api/v1/me/notification-preferences`

`PATCH /api/v1/me/notification-preferences`

## 30.4 Notification Categories

- Security.
- KYC.
- Deposit.
- Transfer.
- Withdrawal.
- Bank payment.
- Conversion.
- Account restriction.
- Support.

## 30.5 Notification Rules

Notifications must not be treated as authoritative evidence of financial settlement.

---

# 31. CUSTOMER SUPPORT APIS

## 31.1 Create Support Ticket

`POST /api/v1/support/tickets`

## 31.2 List Tickets

`GET /api/v1/support/tickets`

## 31.3 Get Ticket

`GET /api/v1/support/tickets/{ticket_id}`

## 31.4 Reply to Ticket

`POST /api/v1/support/tickets/{ticket_id}/messages`

## 31.5 Requirements

- Customer ownership must be enforced.
- Transaction references may be attached.
- Sensitive data must be protected.
- Support agents must not receive unrestricted financial adjustment access.

---

# 32. ADMINISTRATIVE APIS

Administrative endpoints must be separated from customer APIs.

Recommended prefix:

`/api/v1/admin`

## 32.1 Customer Management

`GET /api/v1/admin/users`

`GET /api/v1/admin/users/{user_id}`

## 32.2 KYC Management

`GET /api/v1/admin/kyc/cases`

`GET /api/v1/admin/kyc/cases/{case_id}`

`POST /api/v1/admin/kyc/cases/{case_id}/decisions` accepts decision (approve, reject or request_information), reason and expected case status. Only a permitted KYC reviewer may act on a reviewable case. Return the new case status, reject stale or terminal-case decisions with HTTP 409, and commit the decision and audit evidence atomically. A request for information keeps financial eligibility restricted.

## 32.3 Transaction Monitoring

`GET /api/v1/admin/transactions`

`GET /api/v1/admin/transactions/{transaction_id}`

## 32.4 Risk Management

`GET /api/v1/admin/risk/cases`

## 32.5 Provider Monitoring

`GET /api/v1/admin/providers`

`GET /api/v1/admin/providers/{provider_id}/operations`

## 32.6 Financial Adjustments

`POST /api/v1/admin/financial-adjustments/requests`

## 32.7 Approval Workflow

`POST /api/v1/admin/approvals/{approval_id}/approve`

`POST /api/v1/admin/approvals/{approval_id}/reject`

## 32.8 Administrative Requirements

- Separate permissions.
- Strong authentication.
- Maker-checker for sensitive operations.
- Immutable audit logs.
- No direct balance editing.

---

# 33. TREASURY AND RECONCILIATION APIS

## 33.1 Treasury Overview

`GET /api/v1/admin/treasury/overview`

## 33.2 Provider Balances

`GET /api/v1/admin/treasury/provider-balances`

## 33.3 Reconciliation Runs

`GET /api/v1/admin/reconciliation/runs`

## 33.4 Reconciliation Cases

`GET /api/v1/admin/reconciliation/cases`

## 33.5 Reconciliation Case Details

`GET /api/v1/admin/reconciliation/cases/{case_id}`

## 33.6 Requirements

- Financial data must be access-controlled.
- Reconciliation discrepancies must be auditable.
- Manual settlement changes must require authorization.
- Provider balances must be distinguished from internal ledger balances.

---

# 34. NUVION INTEGRATION ARCHITECTURE

## 34.1 Integration Principle

Nuvion is an external infrastructure provider.

DEYPAY owns the customer-facing API and internal financial orchestration.

## 34.2 Nuvion Adapter

Recommended internal interface:

`NuvionClient`

## 34.3 Adapter Responsibilities

The adapter may support:

- Provider customer/entity creation.
- Identity verification.
- Wallet provisioning.
- Deposit address retrieval.
- Blockchain transfers.
- FX quotes.
- Conversion execution.
- Fiat payouts.
- Provider transaction lookup.
- Webhook verification.

Only confirmed capabilities should be enabled.

## 34.4 Internal Provider Interfaces

Recommended interfaces:

- IdentityVerificationProvider.
- CustodyProvider.
- WalletAddressProvider.
- BlockchainTransferProvider.
- FxQuoteProvider.
- ConversionProvider.
- BankAccountVerificationProvider.
- FiatCollectionProvider.
- FiatPayoutProvider.

## 34.5 Provider Independence

DEYPAY services must depend on internal provider interfaces rather than directly on Nuvion-specific API models.

## 34.6 Provider References

Provider identifiers must be stored separately from DEYPAY identifiers.

## 34.7 Capability Registry

The backend must know which provider supports each:

- Asset.
- Network.
- Country.
- Currency corridor.
- Transaction type.
- Environment.

## 34.8 Nigeria-Specific Validation

The following must be confirmed before production activation:

- Nigerian customer onboarding.
- Nigerian retail KYC support.
- NGN collection.
- NGN conversion.
- Nigerian bank payouts.
- Third-party bank payouts.
- Bank account verification.
- Settlement structure.
- Commercial pricing.
- Legal permissions.

Provider marketing claims alone are insufficient for production authorization.

---

# 35. OTHER PROVIDER INTEGRATION CONTRACTS

## 35.1 Purpose

DEYPAY must support additional providers without redesigning its public APIs.

## 35.2 Provider Selection

Provider selection must consider:

- Capability.
- Availability.
- Country.
- Asset.
- Network.
- Pricing.
- Compliance.
- Operational health.

## 35.3 Fallback

Fallback must not cause duplicate execution.

An uncertain operation must be reconciled before another provider is instructed to execute the same payment.

## 35.4 Provider Data Mapping

Provider-specific statuses must be mapped into canonical DEYPAY transaction states.

---

# 36. WEBHOOK ARCHITECTURE

## 36.1 Provider Webhook Endpoint

Recommended structure:

`POST /api/v1/webhooks/{provider_code}`

## 36.2 Authentication

Provider webhooks must use the provider's supported authentication and signature verification method.

## 36.3 Processing

Webhook processing must:

1. Validate authenticity.
2. Validate payload structure.
3. Identify provider.
4. Persist the event.
5. Deduplicate.
6. Queue processing.
7. Return an appropriate acknowledgment.

## 36.4 Financial Safety

Webhook receipt must not automatically cause financial posting without validating the event against the expected transaction and provider state.

## 36.5 Duplicate Events

Duplicate events must be safe.

## 36.6 Out-of-Order Events

State transitions must handle events arriving out of order.

## 36.7 Recovery

Missing webhooks must be recoverable through provider status lookup and reconciliation.

---

# 37. BACKGROUND JOBS AND ASYNCHRONOUS PROCESSING

## 37.1 Purpose

Long-running financial operations must not depend on a single HTTP request remaining open.

## 37.2 Job Categories

- KYC status synchronization.
- Wallet provisioning.
- Deposit monitoring.
- Withdrawal execution.
- Conversion execution.
- Bank payout execution.
- Notification delivery.
- Reconciliation.
- Provider health checks.

## 37.3 Durable Jobs

Jobs must survive process restarts.

## 37.4 Retry Rules

Retries must be bounded and idempotent.

## 37.5 Unknown Outcomes

Unknown external outcomes require investigation or reconciliation, not blind re-execution.

---

# 38. FINANCIAL TRANSACTION STATE CONTRACTS

## 38.1 Canonical States

Public transaction states:

- created.
- awaiting_authorization.
- pending.
- processing.
- completed.
- failed.
- cancelled.
- under_review.
- pending_reconciliation.
- reversed.
- refunded.

## 38.2 State Semantics

`completed` means the operation has met its defined completion criteria.

`processing` means execution is ongoing.

`pending_reconciliation` means the final outcome is not yet established.

`failed` means a definitive failure has been established.

## 38.3 Internal States

Map the internal financial states in 05-LEDGER-ACCOUNTING.md §25 to public status as follows. Use the operation’s own state for this mapping; component states such as a settled conversion do not complete a bank payment.

| Internal state | Public status |
|---|---|
| CREATED | created |
| VALIDATED, BENEFICIARY_VERIFIED, QUOTED | awaiting_authorization |
| DETECTED, CONFIRMING, SCREENING, AWAITING_FUNDS, AUTHORIZED, RESERVED | pending |
| SUBMITTED, PROCESSING, CONVERTING, PAYOUT_PROCESSING, REFUND_PROCESSING | processing |
| CREDITED, COMPLETED | completed |
| REJECTED, FAILED | failed |
| CANCELLED, EXPIRED | cancelled |
| UNDER_REVIEW, REORG_REVIEW | under_review |
| PENDING_RECONCILIATION | pending_reconciliation |
| REVERSED | reversed |
| REFUNDED | refunded |

## 38.4 Financial Finality

Apply the per-operation evidence rules in 05-LEDGER-ACCOUNTING.md §25.5. A timeout or provider submission acknowledgment alone cannot establish success or failure.

---

# 39. VALIDATION AND BUSINESS RULES

## 39.1 Amount Validation

Amounts must be positive and within supported precision and limits.

## 39.2 Asset Validation

Assets must be enabled.

## 39.3 Network Validation

Network and asset combinations must be supported.

## 39.4 KYC Validation

Customer eligibility must be verified.

## 39.5 Balance Validation

Available balance must be sufficient.

## 39.6 Quote Validation

Quotes must be valid and belong to the authenticated customer.

## 39.7 Beneficiary Validation

Bank beneficiary ownership and verification must be enforced.

## 39.8 Risk Validation

Transactions must satisfy applicable compliance and fraud controls.

---

# 40. PRIVACY AND DATA PROTECTION

## 40.1 Data Minimization

API responses must contain only necessary data.

## 40.2 Sensitive Information

Sensitive personal and financial data must be protected.

## 40.3 Logging

Sensitive request fields must be redacted.

## 40.4 Data Access

Customers must not access other customers' private information.

## 40.5 Nigerian Data Protection

Implementation must account for applicable Nigerian data protection requirements.

---

# 41. RATE LIMITING AND ABUSE PREVENTION

## 41.1 Rate-Limited Operations

- Registration.
- Login.
- Verification challenges.
- Password reset.
- Username lookup.
- Phone discovery.
- Bank account resolution.
- Quote generation.
- Payment execution.

## 41.2 Rate Limit Scope

Limits may apply by:

- IP address.
- Customer.
- Device.
- Session.
- Endpoint.
- Risk tier.

## 41.3 Response

HTTP 429.

## 41.4 Financial Abuse

Financial abuse controls must operate independently of ordinary HTTP rate limits.

---

# 42. OBSERVABILITY AND AUDITABILITY

## 42.1 Request Identification

Every request must have a correlation identifier.

## 42.2 Structured Logging

Logs should include:

- Request ID.
- Operation.
- Result.
- Duration.
- Error code.
- Transaction reference where appropriate.

## 42.3 Metrics

Monitor:

- Request latency.
- Error rate.
- Authentication failures.
- Payment success rate.
- Provider timeout rate.
- Pending transactions.
- Reconciliation discrepancies.

## 42.4 Audit Events

Sensitive operations must create durable audit records.

---

# 43. OPENAPI DOCUMENTATION

## 43.1 OpenAPI Standard

The API must be documented using OpenAPI 3.1 or an approved compatible version.

## 43.2 Documentation Requirements

Each endpoint must define:

- Method.
- Path.
- Description.
- Authentication.
- Request schema.
- Response schema.
- Error responses.
- Examples.
- Idempotency requirements.
- Authorization requirements.

## 43.3 Rust Integration

A Rust OpenAPI library such as Utoipa may be used.

## 43.4 Contract Validation

Generated OpenAPI documentation must remain consistent with actual implementation.

---

# 44. RUST BACKEND IMPLEMENTATION STRUCTURE

## 44.1 Recommended Module Structure

```text
backend/
  crates/
    deypay-api/
      src/
        routes/
        handlers/
        middleware/
        dto/
        errors/

    deypay-application/
      src/
        auth/
        customers/
        compliance/
        wallets/
        transfers/
        payments/
        fx/
        reconciliation/

    deypay-domain/
      src/
        identity/
        wallet/
        ledger/
        payment/
        fx/
        compliance/

    deypay-infrastructure/
      src/
        database/
        repositories/
        providers/
          nuvion/
          banking/
          kyc/
        messaging/
        storage/

    deypay-worker/
      src/
        jobs/
        processors/
        reconciliation/
```

## 44.2 Dependency Direction

API handlers call application services.

Application services use domain rules and interfaces.

Infrastructure implements persistence and external integrations.

## 44.3 Provider Isolation

Provider-specific models must remain inside infrastructure adapters.

## 44.4 Shared Types

Financial amounts, identifiers and statuses must use approved shared domain types.

---

# 45. API TESTING STRATEGY

## 45.1 Unit Tests

Test validation and business rules.

## 45.2 Integration Tests

Test API handlers against real PostgreSQL test databases.

## 45.3 Contract Tests

Validate request and response schemas.

## 45.4 Authentication Tests

Test unauthorized and expired sessions.

## 45.5 Authorization Tests

Test resource ownership and privilege boundaries.

## 45.6 Financial Tests

Test insufficient funds, duplicate requests and transaction limits.

## 45.7 Provider Tests

Use controlled provider mocks and sandbox environments.

## 45.8 Failure Tests

Test:

- Provider timeouts.
- Duplicate webhooks.
- Out-of-order events.
- Worker crashes.
- Database transaction failures.
- Reconciliation recovery.

---

# 46. PERFORMANCE AND RELIABILITY REQUIREMENTS

## 46.1 API Availability

Production availability targets must be approved according to the operating model.

## 46.2 Latency

Ordinary read endpoints should have defined latency targets.

External financial execution must use asynchronous processing where appropriate.

## 46.3 Reliability

Financial correctness must take priority over speed.

## 46.4 Retry Safety

Client retries must not create duplicate financial operations.

## 46.5 Graceful Degradation

If a provider is unavailable, affected financial operations should be disabled or placed into safe states.

Other unaffected application features may remain available.

---

# 47. DEPLOYMENT AND RELEASE MANAGEMENT

## 47.1 Environment Separation

Production credentials must not be used in development.

## 47.2 Configuration

Provider endpoints and capabilities must be environment-specific.

## 47.3 Feature Flags

High-risk financial capabilities should use controlled feature flags.

## 47.4 Rollout

Financial features should be enabled gradually after validation.

## 47.5 Rollback

API deployment rollback must not reverse completed external financial transactions.

---

# 48. IMPLEMENTATION SEQUENCE

## Phase 1: API Foundation

- Axum application.
- Configuration.
- Health endpoints.
- Error handling.
- Logging.
- Authentication middleware.

## Phase 2: Identity

- Registration.
- Login.
- Verification.
- Sessions.
- Profiles.

## Phase 3: Compliance

- KYC.
- Account restrictions.
- Limits.

## Phase 4: Wallets

- Asset registry.
- Wallet profiles.
- Balances.
- Deposit addresses.

## Phase 5: Ledger and Transfers

- Ledger integration.
- Internal transfers.
- Idempotency.

## Phase 6: Provider Integration

- Nuvion adapter.
- Provider capability registry.
- Webhooks.
- Provider operations.

## Phase 7: External Stablecoin Operations

- Deposits.
- Withdrawals.
- Conversion.

## Phase 8: Nigerian Payment Operations

- Bank directory.
- Account verification.
- NGN on-ramp.
- Own-bank off-ramp.
- Pay Anyone.

## Phase 9: Operations

- Transaction history.
- Notifications.
- Support.
- Admin.
- Reconciliation.

## Phase 10: Production Validation

- Security testing.
- Contract testing.
- Financial testing.
- Provider sandbox testing.
- Regulatory launch checks.

---

# 49. RISKS AND MITIGATIONS

## Risk 1: Provider Lock-In

Mitigation: Internal provider interfaces and canonical API models.

## Risk 2: Duplicate Financial Execution

Mitigation: Idempotency and durable workflow records.

## Risk 3: Unauthorized Financial Actions

Mitigation: Strong authentication, authorization and step-up verification.

## Risk 4: Provider Timeout

Mitigation: Explicit unknown states and reconciliation.

## Risk 5: API Contract Drift

Mitigation: OpenAPI and automated contract tests.

## Risk 6: Sensitive Data Exposure

Mitigation: Data minimization and secure logging.

## Risk 7: Regulatory Incompatibility

Mitigation: Production feature gating and legal validation.

---

# 50. OPEN DECISIONS AND DEPENDENCIES

The following require confirmation:

1. Final production domain.
2. Browser session idle and absolute lifetime settings.
3. Step-up authentication mechanism.
4. Username change policy.
5. Supported blockchain networks.
6. Nuvion production capabilities.
7. NGN on-ramp provider.
8. NGN payout provider.
9. Bank account verification provider.
10. Fee schedule.
11. Transaction limits.
12. Quote expiry policy.
13. Provider failover policy.
14. Financial completion criteria.
15. Administrative approval rules.
16. Regulatory launch permissions.
17. Production availability and latency targets.

---

# 51. ACCEPTANCE CRITERIA

The API specification will be accepted when:

1. Every MVP feature has an API contract.
2. Authentication is consistently enforced.
3. Resource ownership is validated.
4. Financial requests are idempotent.
5. Monetary values use exact representations.
6. Transaction states are explicit.
7. Customer balances originate from the ledger.
8. Provider-specific implementation details remain isolated.
9. Nuvion can be integrated through internal adapters.
10. Other providers can be introduced without redesigning the public API.
11. Webhooks can be verified and deduplicated.
12. Unknown financial outcomes can be reconciled.
13. Administrative actions are permission-controlled.
14. API documentation matches implementation.
15. Automated tests validate critical contracts.
16. Production financial capabilities are enabled only after required approvals.

---

# 52. FINAL API ARCHITECTURE DEFINITION

DEYPAY will operate a versioned REST API implemented in Rust using Axum.

The API will serve the responsive PWA and future mobile applications.

All financial operations will be executed through DEYPAY-controlled backend services.

The backend will own customer identity, authorization, KYC enforcement, transaction limits, wallet balances, financial orchestration, ledger posting, transaction history and reconciliation.

External providers, including Nuvion where approved, will be integrated through internal adapters.

Provider-specific identifiers and payloads will not define DEYPAY's public API contracts.

Financial mutation requests will use durable idempotency.

Asynchronous financial operations will use explicit transaction states and recoverable background processing.

Customer balances will originate from DEYPAY's double-entry ledger.

Nigerian bank payments will require approved conversion and payout infrastructure.

Sensitive operations will use appropriate authorization and audit controls.

The API will be documented through OpenAPI and validated through automated tests.

**FINAL API ARCHITECTURE PRINCIPLE**

The frontend requests.

The DEYPAY backend validates.

The financial ledger records.

The provider executes.

The reconciliation system verifies.

The customer receives an accurate result.

No external infrastructure provider should control the complete DEYPAY product simply because DEYPAY uses its APIs.

---

**END OF DOCUMENT**

**DEYPAY API SPECIFICATION AND CONTRACT DESIGN**

**Document ID:** DP-API-004  
**Version:** 1.0  
**Status:** Proposed Engineering Baseline — Pending Final Validation
