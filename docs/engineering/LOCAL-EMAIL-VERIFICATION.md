# Local email-verification outbox (IMP-P3-002 workstation acceptance)

Use this only to finish account and sign-in testing on a workstation when email cannot be sent. It does not replace, and is not evidence of, external email delivery. IMP-P3-002 stays In testing until a real Resend message is delivered in staging and confirmed.

## What it is

`backend/crates/deypay-worker/examples/local_verification_outbox.rs` is a Cargo example. It runs the production `VerificationWorker` path once: dispatch, claim, HMAC token generation, prepare and finish. Instead of an email provider, it writes each verification email to a private directory. You then open the link, and the real `/verify-email` page and `POST /api/v1/auth/verify-email` redeem the token. Nothing writes verification evidence directly.

## Guards

- `APP_ENV` must be `local` or `test`. Staging and production exit with an error.
- `EMAIL_VERIFY_BASE_URL` must be a loopback `/verify-email` page, for example `https://127.0.0.2:8443/verify-email`.
- The worker runs with the `sandbox` scope. `integrations.claim_verification_email` returns only customers whose legal acceptance is sandbox-scoped, so published-scope customers are never processed.
- Each run uses a fresh random HMAC key that never leaves the process. If a run stops between claim and finish, the reclaimed job is dead-lettered as `MESSAGE_CHANGED`, and the customer must request a resend.
- Cargo examples are not part of `cargo build` output or any deployable binary.
- It connects with the dedicated worker database role, which has only the four worker functions (`email-verification-worker-grants.psql`).

## Run

1. Start the API with `EMAIL_VERIFICATION_ENABLED=true` so it accepts verification and resend requests. Register the account through the real API or frontend.
2. Create an outbox directory that only the operator can read. On Windows, files inherit the directory ACL. On Unix, files are created with mode `0600`. Keep it outside the repository.
3. Build and run the example:

   ```sh
   cargo build -p deypay-worker --example local_verification_outbox
   APP_ENV=local DATABASE_URL=<worker-role URL> \
   EMAIL_VERIFY_BASE_URL=https://127.0.0.2:8443/verify-email \
   DEYPAY_LOCAL_OUTBOX_DIR=<absolute private directory> \
   target/debug/examples/local_verification_outbox
   ```

   It processes all claimable sandbox verification jobs, up to 100 per run, then exits. Each message is written to `<message-id>.txt`. Output and logs contain message IDs and outcomes only, never links or tokens.
4. Open the link from the message file in the browser and select **Verify email**. The link is single use and expires after the configured TTL (15 minutes by default). Delete the file once it has been used.

Do not paste links into chats, tickets or logs. Never use this against a hosted database.
