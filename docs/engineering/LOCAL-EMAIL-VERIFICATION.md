# Local email outbox: verification and password reset (workstation acceptance)

Use this only to test accounts on a workstation when email cannot be sent. It covers email verification (IMP-P3-002) and password recovery (IMP-P3-008). It does not replace, and is not evidence of, external email delivery. IMP-P3-002 stays In testing until a real Resend message is delivered in staging and confirmed.

## What it is

`backend/crates/deypay-worker/examples/local_verification_outbox.rs` is a Cargo example. It runs the production `VerificationWorker` once and, when `PASSWORD_RESET_BASE_URL` is set, the production `RecoveryWorker` too. Each worker goes through dispatch, claim, HMAC token generation, prepare and finish. Instead of an email provider, the example writes each email to a private directory. You then open the link. The real `/verify-email` page and `POST /api/v1/auth/verify-email` redeem a verification token; the real `/reset-password` page and `POST /api/v1/auth/reset-password` redeem a reset token. Nothing writes verification, reset or session evidence directly.

## Guards

- `APP_ENV` must be `local` or `test`. Staging and production exit with an error.
- `EMAIL_VERIFY_BASE_URL` must be a loopback `/verify-email` page, for example `https://127.0.0.2:8443/verify-email`. If set, `PASSWORD_RESET_BASE_URL` must be a loopback `/reset-password` page.
- Both workers run with the `sandbox` scope. The claim functions return only customers whose legal acceptance is sandbox-scoped, so published-scope customers are never processed.
- Each worker uses a fresh random HMAC key per run that never leaves the process. If a run stops between claim and finish, the reclaimed job is dead-lettered as `MESSAGE_CHANGED`, and the customer must request the email again.
- Cargo examples are not part of `cargo build` output or any deployable binary.
- It connects with the dedicated worker database role. That role has only the worker functions in `email-verification-worker-grants.psql` and `password-recovery-worker-grants.psql`.

## Run

1. Start the API with `EMAIL_VERIFICATION_ENABLED=true` so it accepts verification and resend requests. For password recovery, start the verifier (`deypay-auth`) with `PASSWORD_RECOVERY_ENABLED=true`. Without it, `forgot-password` and `reset-password` return 503 ("temporarily unavailable").
2. Register the account, or request a reset, through the real API or frontend.
3. Create an outbox directory that only the operator can read. On Windows, files inherit the directory ACL. On Unix, files are created with mode `0600`. Keep it outside the repository.
4. Build and run the example:

   ```sh
   cargo build -p deypay-worker --example local_verification_outbox
   APP_ENV=local DATABASE_URL=<worker-role URL> \
   EMAIL_VERIFY_BASE_URL=https://127.0.0.2:8443/verify-email \
   PASSWORD_RESET_BASE_URL=https://127.0.0.2:8443/reset-password \
   DEYPAY_LOCAL_OUTBOX_DIR=<absolute private directory> \
   target/debug/examples/local_verification_outbox
   ```

   It processes all claimable sandbox jobs, up to 100 rounds per run, then exits. Each message is written to `<message-id>.txt`. Output and logs contain message IDs and outcomes only, never links or tokens.
5. Open the link from the message file in the browser:
   - For verification, select **Verify email**. The link expires after 15 minutes by default.
   - For a reset, choose a new password. The link expires after 30 minutes by default, and resetting signs the account out everywhere.

   Each link works once. Delete the file after use.

Do not paste links into chats, tickets or logs. Never use this against a hosted database.
