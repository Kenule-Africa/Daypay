# IMP-P3-012 authentication admission

## Cycle 2 integration contract proposed for coordination

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

The subject must never contain a bearer credential, full reset token, password or arbitrary request body. The handler owns parsing. The limiter charges the operation independently of account existence. Limit values for new operations come from database policy and require owner configuration before activation.

Login retains its verifier-owned release and identifier budgets. Email verification retains the independent resend and verify budgets, resend cooldown and challenge attempt limits. Registration requires a shared deployment-wide counter before password screening or hashing.
