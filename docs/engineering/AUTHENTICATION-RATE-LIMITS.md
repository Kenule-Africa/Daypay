# IMP-P3-012 authentication admission

## Approved Cycle 2 integration contract

Claude owns the forgot-password, reset-password and username-availability handlers. Codex supplies the following two asynchronous functions in `deypay_infrastructure::rate_limit`.

```rust
admit(pool: &PgPool, operation: AuthOperation) -> Result<(), AdmissionError>
admit_subject(pool: &PgPool, operation: AuthOperation, subject: &str) -> Result<(), AdmissionError>
```

1. Call `admit` before reading the body or performing database lookups. Exhaustion returns `AdmissionError::Limited`. Respond with HTTP 429 and the existing generic rate-limit envelope.
2. After bounded parsing and validation, call `admit_subject` before account lookup, token verification, delivery or username availability queries. Charge recognized and unrecognized subjects identically.
3. Map `AdmissionError::Unavailable` at either stage to generic HTTP 503. Stop processing on any admission error and return `Cache-Control: no-store`.

| AuthOperation variant | Subject supplied by the handler | Subject limit reached |
|---|---|---|
| `ForgotPassword` | Canonical registration email | Return the same acknowledgement as a successfully submitted request, without sending mail |
| `ResetPassword` | Public challenge selector | Return generic HTTP 429 |
| `UsernameAvailability` | Canonical handle | Return generic HTTP 429 |

The subject must never contain a bearer credential, full reset token, password or arbitrary request body. The handler owns parsing. The limiter charges the operation independently of account existence. The reset handler supplies the canonical, non-secret selector used by its lookup. The limiter preserves selector case. Limit values for new operations come from database policy and require owner configuration before activation.

Login retains its verifier-owned release and identifier budgets. Email verification retains the independent resend and verify budgets, resend cooldown and challenge attempt limits. Registration requires a shared deployment-wide counter before password screening or hashing.

## Operator setup

Apply migration `202611251201`, then reapply the registration and authentication grant scripts and apply `infrastructure/postgres/public-admission-grants.psql`. Existing email grants remain valid. The runtime roles have execute-only admission access. Apply grants and policy before rolling out the new executables.

As the database policy administrator, configure `identity.admission_policy(operation,total_limit,subject_limit)` using deployment-approved per-minute limits. Supply both limits for `forgot_password`, `reset_password` and `username_availability`. Supply only the total for `refresh`, `session_list`, `logout` and `session_revoke`. Each policy generates a private subject HMAC key. Application logs and audit messages must exclude subjects, keys, selector values and SQL bind arguments. Disable SQL bind-value logging on the database, connection proxy and tracing collector (PostgreSQL `log_parameter_max_length=0` and `log_parameter_max_length_on_error=0`). No production limits are seeded by the migration. Removing a required row closes that operation with HTTP 503.

Refresh and listing have independent budgets. Logout and targeted revocation each reserve a separate allowance for successful session closure. Invalid, expired, foreign and repeat attempts share the operation's unsuccessful-attempt allowance. Both allowances use its configured total. Cookie clearing without a presented session requires no database access. A denied database mutation rolls back its audit event and session changes together. Size both closing budgets for legitimate peak traffic and exercise account-security operations before rollout.

Fixed windows use the PostgreSQL clock after locking the operation row. A tighter registration or email limit takes effect on the next attempt and stays in force until the next minute. An operation retains at most one window of subject digests, bounded by its configured total. The next minute's first admission deletes the expired subjects for that operation. Rollout and CI do not grant release approval.
