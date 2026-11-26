# Phase 5 ledger: independent review remediation

PR #18 · branch `feat/claude-phase5-financial-ledger` · reviewed commit `85a5e71`

This document records how each review finding was classified and resolved. Each resolution keeps the existing ledger architecture, exact arithmetic, double-entry invariants, immutable journals, reservations and reconciliation unchanged.

Files changed:
- the migration `backend/migrations/202611260001_financial_ledger.sql`, referred to below as **the migration**;
- `infrastructure/postgres/ledger-roles.psql` and `ledger-grants.psql`;
- the ledger adapter `deypay-infrastructure/src/ledger.rs`;
- the application contract `deypay-application/src/ledger.rs`;
- the PostgreSQL suite `deypay-infrastructure/tests/ledger.rs`.

Tests named below are in that suite.

| # | Finding | Classification | Resolution |
|---|---|---|---|
| 1 | FinancialGate bypass through the poster role | **Confirmed**, including system postings that create customer entitlement | Database-enforced activation registry, fail-closed by default |
| 2 | Maker-checker identity spoofing | **Confirmed** | Operator identity bound to the authenticated database login. Fail-closed until logins are bound; depends on administrative authentication (IMP-P16-001) |
| 3 | Reservation and settlement evidence | **Confirmed** | Separate evidence role and authenticated provider outcomes. Settlement and post-submission release are unavailable until an adapter holds that role |
| 4 | Privilege and migration integrity | **Mostly mitigated by existing controls.** Verification gaps were confirmed | Grant verification extended; restricted install proven; residual risks recorded below |
| 5 | Concurrency and accounting invariants | **Re-verified** | 18 PostgreSQL tests (14 extended, 4 new) |

## Finding 1: FinancialGate bypass

**Confirmed.** `ledger-grants.psql` gave `deypay_ledger_poster` EXECUTE on `ledger.transfer_between_customers` and `ledger.reserve_funds`. Inside PostgreSQL those functions enforced only the baseline:
- the customer is `KYC_APPROVED`;
- there is no active restriction;
- the wallet is `ACTIVE`;
- the asset is not on hold.

`NoApprovedPolicy` lived only in Rust, so a process holding the poster role could spend customer funds by calling SQL directly.

The review also found a second problem: `ledger.post_journal` could credit any customer's available account against custody or suspense. That manufactures customer entitlement without funding evidence. Only reconciliation would have detected it.

**Resolution:**
- **Activation registry.** The migration adds `ledger.capability_activations`, which only the database administrator can write. A row grants one capability for one asset:
  - `CUSTOMER_CREDIT`, `CUSTOMER_TRANSFER` or `CUSTOMER_RESERVATION`;
  - each row records the approval reference and a mandatory per-operation ceiling.
- **No default.** Nothing is seeded, so every environment starts fail-closed.
- **History only.** Rows can be deactivated but never edited or deleted, enforced by the `activation_history` and `no_activation_delete` triggers.
- **Owner cannot write it.** The functions' owner `deypay_ledger_owner` has SELECT only, and grant verification aborts if that ever changes.
- **Checks in the database.** `ledger.require_capability` (LE014 → `LedgerError::NotActivated`) runs in:
  - `transfer_between_customers` and `reserve_funds`;
  - `apply_journal` for every SYSTEM-mode line that increases a customer account.

  Replays of postings that were already committed still return their original result.
- **Attribution.**
  - ADMIN actor attribution is limited to approved corrections (LE012).
  - A `CUSTOMER` actor may move only that customer's own funds (LE006).
- **Enabling after approval.** Once the eligibility policy and limits are approved, the DBA inserts one row per capability and asset through change control. To disable, the DBA sets `deactivated_at`, `deactivated_by` and `deactivation_reason`. The application gate (`NoApprovedPolicy`) is replaced by the approved policy separately; both layers must allow a movement.
- **Test fixtures.** Fixtures activate capabilities the same way, as the administrator, inside automated tests only.

**Evidence:**
- `customer_movements_are_fail_closed_until_activated`:
  - Direct SQL as the poster is refused with LE014 for transfers, reservations and system credits to customers. So are the same calls through the adapter.
  - ADMIN attribution and impersonated customer actors are refused.
  - Denied operations write no record of any kind.
  - Platform-only postings still work.
  - A new activation enforces its ceiling.
  - Activation history cannot be rewritten.
  - `NoApprovedPolicy` still refuses on its own.
- `runtime_roles_cannot_bypass_the_posting_interface`: no runtime role, nor `deypay_ledger_owner`, can write activations.

## Finding 2: Maker-checker identity spoofing

**Confirmed.** Every controller function took caller-supplied operator UUIDs and checked only that each UUID held a permission. One controller principal could therefore name two administrators as requester and approver and complete a correction alone. Administrative authentication (IMP-P16-001, IMP-P16-002) is still in the backlog, so no MFA-backed admin session exists to bind to.

**Resolution:** bind the operator to the authenticated database login. The login is the only verifiable identity available at the actual privilege boundary.
- **Binding table.** `ledger.operator_logins` maps a login (`session_user`) to one operator. Only the database administrator writes it; rows can be revoked but never edited or deleted.
  - `session_user` is fixed when the connection authenticates.
  - `SET ROLE` and SECURITY DEFINER change only `current_user`.
  - A non-superuser cannot change `session_user`.
- **Claims are checked.** `ledger.session_operator` and `ledger.authorize_operator` (LE016 → `LedgerError::OperatorNotAuthenticated`) treat the stated UUID as a claim. It must equal the bound operator, who must still hold an unrevoked permission and not be SUSPENDED or CLOSED.
- **Coverage.** Every operator entry point uses this check:
  - `open_platform_account`, `set_account_status`;
  - `request_correction`, `decide_correction`, `execute_correction`;
  - `resolve_recovery_case`, `lift_asset_hold`;
  - `start_reconciliation`, `update_case`;
  - every reconciliation step, through `operations.running`. Only the operator who started a run can continue it.
- **Audit.** Every audit event now records the `database_login`.
- **Fail-closed.** With no binding, every operator action fails.

**Activation dependency:**
- **Who may be bound.** Bind only logins whose credentials are issued to that one person through the approved privileged-access process with strong MFA (SEC-ADM-002, SEC-ADM-010), for example short-lived per-operator credentials. A shared service login must never be bound.
- **When.** Until IMP-P16-001 provides that process, production should bind no logins, so corrections and reconciliation stay fail-closed.
- **What it does not prevent.** Maker-checker cannot stop two colluding operators.

**Evidence:**
- `operator_identity_is_bound_to_the_authenticated_session`:
  - An unbound controller login cannot act as any stated operator.
  - The maker cannot approve or execute by naming the checker (forged identity).
  - The maker cannot `SET ROLE` to the checker's login or rebind its own login.
  - Self-approval is refused.
  - A revoked admin assignment blocks approval (revoked permission).
  - An operator without permission cannot execute (unauthorized execution).
  - A revoked binding ends authority immediately, and binding history is immutable.
  - Another operator cannot continue someone else's reconciliation run.
  - The audit trail shows the maker and checker logins.
- `reversals_compensate_with_maker_checker_approval` and `reconciliation_records_every_discrepancy` run the authorized paths from two separately bound sessions.

## Finding 3: Reservation and settlement evidence

**Confirmed.** A poster's own word was accepted for three things:
- `release_reservation` accepted `DEFINITIVE_NON_EXECUTION` with any reference while the state was SUBMITTED or UNKNOWN;
- `consume_reservation` accepted `VERIFIED_SETTLEMENT` with any reference;
- `record_execution_state` let the poster set EXECUTED.

The existing state rules stopped honest mistakes, such as cancelling after submission, but not a caller asserting a false outcome.

**Resolution:**
- **Evidence boundary.** The migration adds `ledger.provider_outcomes` and `ledger.record_provider_outcome`, executable only by the new NOLOGIN role `deypay_ledger_evidence`. That role is for authenticated provider adapters. An outcome:
  - must name the `integrations.provider_operations` row registered for the reservation's own transaction (SEC-WHK-005), and stores the provider reference, the SHA-256 digest of the verified message and the observation time;
  - is limited to one per reservation and one per provider operation;
  - replays identically, and a contradicting outcome is refused (LE004);
  - cannot be changed or deleted.
- **Settlement.** `consume_reservation` requires the EXECUTED outcome.
- **Release.**
  - `DEFINITIVE_NON_EXECUTION` requires the NOT_EXECUTED outcome.
  - `SETTLEMENT_REMAINDER` requires the EXECUTED outcome.
  - `PERMITTED_CANCELLATION` remains possible only before submission.
  - Missing evidence raises LE015 (`LedgerError::EvidenceRequired`).
- **Workflow states.** `record_execution_state` accepts only SUBMITTED and UNKNOWN, and nothing once an outcome is definitive. `LedgerService` refuses `Executed` before reaching the store.
- **Movements cite their evidence.** Each reservation movement carries a `provider_outcome_id`. A foreign key ties it to the same reservation, and a CHECK requires it for settlement, confirmed non-execution and remainder releases.
- **Separation of duties.**
  - At run time, `ledger.require_separated_duty` refuses a non-superuser session that holds both the poster and evidence roles, on either side (LE012).
  - At grant time, grant verification refuses any login holding both.

**Not claimed:**
- No provider adapter exists, and no signed receipt or provider integration was created.
- Verifying a provider's signature or status query is the adapter's job before it calls `record_provider_outcome`.
- `deypay_ledger_evidence` is granted to no login, so settlement and post-submission release are unavailable until an authenticated adapter is approved and granted the role.

**Evidence:**
- `settlement_and_release_need_authenticated_provider_evidence`:
  - Forged releases and settlements are refused through the adapter and through direct SQL, and write nothing.
  - The poster cannot reach the evidence boundary.
  - Evidence is refused for another transaction's provider operation, or for a reservation with no transaction.
  - The dual-role login is refused on both sides.
  - Identical replays are idempotent; contradictions and reuse of a provider operation are refused.
  - After a definitive outcome the workflow cannot change state, and settlement is impossible after NOT_EXECUTED.
  - The release cites the outcome, and outcomes are immutable.
- `reservations_follow_evidence_not_timeouts` covers the complete lifecycle with evidence, and checks that all four evidenced movements cite their provider outcome.
- `randomized_operations_preserve_every_invariant` settles only against recorded outcomes.

## Finding 4: Privilege and migration integrity

| Item | Status before | Change and evidence |
|---|---|---|
| PUBLIC function execution | Mitigated: the migration revokes all ledger and operations functions from PUBLIC, and grant verification fails if any routine is PUBLIC-executable | Unchanged. **Residual:** a later migration's new function is PUBLIC-executable until that migration revokes it. Runtime roles with schema USAGE could call it in that window. Rule: every ledger migration ends with the REVOKE and is followed by `ledger-grants.psql`, which refuses foreign-owned or PUBLIC routines. A global `ALTER DEFAULT PRIVILEGES … REVOKE EXECUTE ON FUNCTIONS FROM PUBLIC` for the migration owner is recommended to Platform; it was not changed here because it affects every module |
| Direct runtime table writes | Mitigated | Verification now also covers `deypay_ledger_evidence`. `deypay_ledger_owner` cannot write the trust anchors (`runtime_roles_cannot_bypass_the_posting_interface`) |
| SECURITY DEFINER ownership and search_path | Mitigated: every routine was already pinned and owned by `deypay_ledger_owner` | Verification now fails if any ledger or operations routine lacks `search_path=pg_catalog, pg_temp` |
| Privilege inheritance | **Gap:** memberships were never checked | Verification refuses: a runtime role, or a login holding one, that is a member of `deypay_ledger_owner`; `deypay_app` holding any ledger role; a login holding both poster and evidence |
| Unauthorized internal-function access | Mitigated by the REVOKE | Verification now enforces an exact per-role allowlist. Internal helpers such as `apply_journal`, `claim_event`, `session_operator`, `require_capability` and `record_item` are reachable by no runtime role (tested) |
| Install with restricted roles | **Not demonstrated** | `restricted_roles_install_the_ledger`: a non-superuser migration administrator, a member of `deypay_ledger_owner` with CREATE on the database, runs every migration and the grants. Verification passes, routines are owner-owned, and activations, bindings and journals start empty |
| Transactional consistency | Mitigated: migration, grants and each adapter call are single transactions; balancing is checked at commit | Unchanged; `failures_and_crashes_leave_no_partial_posting` now checks every record table |
| Immutable posted history | Mitigated for runtime roles, the function owner and table-owner DML | Provider outcomes, activations and bindings are added under the same triggers. **Residual:** the table owner (the migration principal) can alter tables or disable triggers, inherently in PostgreSQL. Its credential must be limited to the change-controlled deployment pipeline; drift is detected by `integrity_violations()` and reconciliation |

## Finding 5: Invariants re-verified

| Invariant | Tests |
|---|---|
| Journals balance per asset; invalid journals never post | `invalid_journals_never_post`, `posted_history_is_immutable_and_unbalanced_journals_cannot_commit` |
| No customer double-spend | `parallel_spending_cannot_overdraw` (20 concurrent reservations, 40 crossing transfers) |
| Concurrent duplicate delivery posts once | `concurrent_duplicate_deliveries_post_once` (25 deliveries) |
| Available and reserved move atomically | `reservations_follow_evidence_not_timeouts`, `randomized_operations_preserve_every_invariant` |
| Unknown outcomes stay protected | `reservations_follow_evidence_not_timeouts`, `settlement_and_release_need_authenticated_provider_evidence` |
| Idempotent release and settlement | Both tests above |
| Compensation rather than mutation | `reversals_compensate_with_maker_checker_approval` |
| Underfunding is detected | `reconciliation_records_every_discrepancy` (shortfall raises a critical case and an asset hold) |
| Crash and rollback leave nothing | `failures_and_crashes_leave_no_partial_posting` |
| Maker-checker | `reversals_compensate_with_maker_checker_approval`, `operator_identity_is_bound_to_the_authenticated_session` |
| Denied operations create no accounting records | The Finding 1–3 tests and the crash test compare ten record tables before and after: events, journals, entries, reservations, movements, outcomes, corrections, recovery cases, outbox and audit |

## Unresolved dependencies

None of these is a ledger defect; each is an activation the ledger deliberately refuses until it exists.

- **Eligibility policy and limits.** An approved policy and limits are needed, then activation rows per capability and asset, plus a `FinancialGate` replacing `NoApprovedPolicy`.
- **Administrative identity.** Administrative authentication and RBAC with strong MFA (IMP-P16-001, IMP-P16-002) and per-operator database credentials are needed before any operator login is bound.
- **Evidence adapters.** Authenticated provider adapters that verify provider messages are needed before `deypay_ledger_evidence` is granted.
- **Other approvals and setup:**
  - DEC-015 Finance chart mapping and fee and rounding policy;
  - DEC-004 custodian;
  - the production asset registry;
  - the first reconciliation against a real provider;
  - service-login grants;
  - Platform adoption of the default-privilege recommendation.

## Verification

Local checks were `cargo fmt --check` and Clippy with `-D warnings` on the ledger crates and their tests. The full suites ran in GitHub Actions; the exact commit and run are recorded in PR #18.
