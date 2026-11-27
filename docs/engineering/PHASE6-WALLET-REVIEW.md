# Phase 6 wallets: independent review remediation

PR #20 · branch `feat/claude-phase6-wallet-asset-management` · reviewed commit `0a283f3`

**Status:** PR #20 merged to `main` at `1386d1b` on 2026-11-27; push run [45935622358](https://github.com/Kenule-Africa/Daypay/actions/runs/45935622358) passed. IMP-P6-001–010 are Completed — Engineering Accepted. This record describes the review as it stood before the merge. Migration `202611261401` is now immutable, so any further change must be a new forward-only migration.

This document records how each review finding was classified and resolved. The resolutions keep the Phase 6 architecture unchanged: ledger-only balances, DBA-controlled registries, the provider-independent `DepositAddressProvider` port and the role model. They add no state, role, grant, endpoint, provider capability or task ID.

Files changed:
- the migration `backend/migrations/202611261401_wallet_asset_management.sql`, referred to below as **the migration**;
- the PostgreSQL suite `backend/crates/deypay-infrastructure/tests/wallets.rs`;
- this record, and the related documentation listed in [PROGRESS.md](PROGRESS.md).

Tests named below are in that suite.

| # | Finding | Classification | Resolution |
|---|---|---|---|
| 1 | `wallets.begin_provisioning` does not revalidate eligibility at execution time | **Confirmed** | One eligibility rule, `wallets.deposit_blocker`, applied when the request is opened and again when the worker begins. Lock order is wallet, then request. SUBMITTED and UNKNOWN requests stay lookup-only |
| 2 | `wallets.select_address_provider` precedence is not deterministic and can override restrictions | **Confirmed** | `wallets.eligible_address_providers` applies one documented precedence. Explicit restrictions win at every scope, and rows that disagree at the same scope exclude the provider |
| 3 | Verification items 1–12 | **No further defect found** | Evidence is in the table below. Phase 9 requirements are recorded for late or unsolicited deposits, token precision and the deposit-address QR |

## Finding 1: execution-time revalidation

**Confirmed.** `request_deposit_address` checked eligibility only when the request was opened. `begin_provisioning` then checked only that the request's provider was still the one `select_address_provider` ranked first.

**Root cause.** Eligibility was evaluated at request time only. Suppose any of these happened between the request and the worker run:
- the wallet was suspended;
- KYC lapsed;
- a compliance restriction was applied;
- the asset was suspended or put on hold;
- `CUSTOMER_CREDIT` was deactivated;
- the network or token contract was disabled.

The worker would still have asked the provider for an address for a wallet that can no longer receive.

**Second defect.** Because the check compared against the top-ranked provider only (`LIMIT 1`), adding a higher-priority provider failed every pending request to the original, still-eligible provider.

**Not affected:**
- An address assigned to a wallet that cannot receive was already hidden by `wallets.wallet_network_view`.
- Provisioning never posts to the ledger.
- SUBMITTED and UNKNOWN requests were already resolved only by lookup with their original provider.

**Resolution:**
- **One rule.** `wallets.deposit_blocker(wallet, asset_network)` returns the first reason a wallet cannot receive on an asset network, or NULL. The checks run in this order:
  1. wallet status;
  2. account and compliance restriction;
  3. asset suspension;
  4. asset hold;
  5. KYC;
  6. network and contract availability;
  7. deposit flags and `CUSTOMER_CREDIT` activation.

  `request_deposit_address` now uses it. Customers see the same error codes as before: WA006, WA012 and WA007.
- **Revalidation.** `begin_provisioning` applies the same rule to a PENDING request. It also requires the request's own provider to be among `eligible_address_providers` for the asset network, so a newly preferred provider no longer fails existing requests. If anything was revoked:
  - the request becomes FAILED, with the reason in `last_error_code` (for example `WALLET_NOT_RECEIVABLE`, `KYC_REQUIRED`, `ASSET_ON_HOLD`, `DEPOSITS_NOT_ENABLED` or `PROVIDER_NOT_SELECTABLE`);
  - nothing is submitted and the record stays.

  FAILED is a closed request, so the customer can open a new one once the cause is cleared.
- **Concurrency.**
  - `begin_provisioning` locks the wallet row (FOR SHARE) before the request row (FOR UPDATE). This is the same wallet-first order that `request_deposit_address` and `set_wallet_status` use (FOR UPDATE). A status change therefore either commits before the check or waits until the SUBMITTED mark commits.
  - Other revocations (KYC, compliance, holds, activations, registry rows) take effect from the moment they commit; each statement reads a fresh snapshot under READ COMMITTED.
  - `lock_timeout = '10s'` bounds the wait.
- **After submission.** A revocation that lands after the SUBMITTED mark cannot recall an in-flight provider call. The request is then handled as an attempt that may have been received:
  - it is looked up only with its original provider and never resubmitted;
  - its result is recorded as evidence;
  - any address stays hidden until the wallet may receive again.

  Visibility gating and the Phase 9 credit rules carry the safety from that point. They do not depend on preventing the call.

**Evidence** (`provider_submission_revalidates_eligibility`):
- **Revocations between request and worker run.** Each of these fails the request with its reason, makes no provider call and records no address:
  - KYC lapse;
  - compliance restriction;
  - asset suspension;
  - asset hold;
  - `CUSTOMER_CREDIT` deactivation;
  - contract deposits disabled;
  - network disabled;
  - capability withdrawn;
  - operator wallet suspension.
- **Preferred provider added later.** A pending request is still submitted to its own eligible provider; the new provider is not contacted.
- **Suspension after an UNKNOWN outcome.** The wallet is suspended after the outcome:
  - the worker only looks the request up;
  - the address is recorded with its original provider;
  - the customer sees `Unavailable`;
  - the address reappears after reactivation.
- **Suspension racing the worker.** A worker started while an uncommitted suspension holds the wallet row waits for it. After the commit it returns `Done` with `WALLET_NOT_RECEIVABLE`.
- No journal is written and `ledger.integrity_violations()` stays empty.

## Finding 2: provider precedence

**Confirmed.** `select_address_provider` used `DISTINCT ON (provider)`, ordered by how specific each capability row was, and then checked only that single row.

**Root causes:**
- **Arbitrary tie.** Asset-only and network-only rows scored the same, so when a provider had both, PostgreSQL could pick either. The result depended on physical row order, so an equally specific `UNSUPPORTED` or non-operational row could lose to an approval.
- **Ignored health.** A provider-wide `UNSUPPORTED` or non-operational row was ignored whenever a more specific `PRODUCTION_ENABLED` row existed. A provider marked down everywhere could still be routed.

**Resolution.** `wallets.eligible_address_providers(asset_network)` returns every eligible provider in routing order, and `select_address_provider` takes the first. Precedence is evaluated among a provider's applicable rows: production environment, `DEPOSIT_ADDRESS_PROVISIONING`, no corridor or country scope, and matching or NULL asset and network.

1. **Explicit restrictions win.** An `UNSUPPORTED` or non-operational row at any applicable scope excludes the provider. No broader, narrower or equally specific approval overrides it.
2. **The most specific scope decides.** The scopes, from most to least specific:
   - exact asset and network;
   - asset-only or network-only, which are equal;
   - provider-wide.

   Every row at the deciding scope must be `PRODUCTION_ENABLED`, production-approved and available. Rows that disagree at the same scope exclude the provider.
3. **The provider must be approved.** It must be `APPROVED` and enabled. Candidate, suspended, retired and disabled providers are never routed.
4. **Order.** Providers are ordered by the lowest routing priority among the deciding rows, then by provider code. The result is deterministic.

Sandbox rows and corridor- or country-scoped rows never route production deposit addresses.

**Evidence** (`capability_precedence_is_deterministic_and_restrictive`). The test uses overlapping provider-wide, asset-only, network-only and exact rows across USDC and USDT on two networks. It also includes:
- the two equally specific rows recorded in both orders;
- restrictions at broad and narrow scope;
- a provider that is down;
- suspended, retired and disabled providers;
- sandbox and corridor rows;
- a priority tie.

It asserts:
- the full eligible list and the selection for each asset network, repeated 20 times;
- a customer request routed by the same rule;
- a restriction that applies only where its scope reaches;
- no provider at all once nothing is approved.

## Verification items

| # | Item | Result | Evidence |
|---|---|---|---|
| 1 | Ledger use | Verified | <ul><li>Wallet functions reach the ledger only through `open_customer_accounts`, `customer_balance`, `asset_controls` and `authenticated_operator`.</li><li>`wallet-grants.psql` aborts if `deypay_wallet_owner` gains any ledger table privilege.</li><li>The provisioning tests assert no journal and no integrity violation.</li></ul> |
| 2 | No fabricated balances | Verified | <ul><li>The wallet schema has no balance column.</li><li>`customer_wallets` returns `ledger.customer_balance` exactly, or NULL while ledger accounts are being opened.</li><li>An unavailable store returns 503, and the web app shows no number.</li><li>Tests: `balances_come_only_from_the_ledger`, `wallet_http`, frontend.</li></ul> |
| 3 | Ownership and session authorization | Verified | <ul><li>Every route takes the customer from `AuthenticatedCustomer`.</li><li>Each SQL function resolves a wallet only together with that customer (`owned_wallet`, WA004).</li><li>Foreign, unknown and malformed wallets return the same 404.</li><li>The POST route checks Origin.</li><li>Covered by PostgreSQL and HTTP cases.</li></ul> |
| 4 | Roles and SECURITY DEFINER | Verified after the change | <ul><li>The two new helpers are SQL functions without SECURITY DEFINER.</li><li>Their `search_path` is pinned, they are owned by `deypay_wallet_owner`, revoked from PUBLIC and granted to no role.</li><li>The `wallet-grants.psql` checks pass in CI with them: ownership, PUBLIC execution, pinned `search_path`, memberships and per-role allowlists.</li><li>No grant changed.</li></ul> |
| 5 | Lifecycle | Verified | <ul><li>Wallet, request and address transitions are enforced by triggers, and identities are immutable.</li><li>Revalidation uses the existing FAILED state, with a reason code.</li></ul> |
| 6 | Late or unsolicited deposits | Phase 9, foundations verified | <ul><li>Phase 6 has no deposit intake.</li><li>Addresses are never deleted or edited, and retired, suspended and rejected records keep their wallet, provider, request and evidence.</li><li>One on-chain address and memo on a network belongs to one customer and provider for ever, across every status (`guard_wallet_address`, serialized by an advisory lock).</li><li>Requirements for Phase 9 are recorded below.</li></ul> |
| 7 | Idempotency | Verified | <ul><li>Wallet creation is idempotent and safe under concurrency.</li><li>A repeated address request returns the open request (`one_open_provisioning`).</li><li>`deypay-addr-<uuid>` is unique and sent as the idempotency key.</li><li>Repeating a final result is a no-op, and a contradicting one is refused (WA009).</li><li>Verification is idempotent.</li></ul> |
| 8 | Portability and historical references | Verified | <ul><li>Each request and address keeps its `provider_id` and provider references.</li><li>Routing changes affect only new requests, and a pending request keeps its own eligible provider.</li><li>Addresses are retired, not replaced.</li><li>Registry changes are append-only in `wallets.registry_changes`.</li></ul> |
| 9 | USDC/USDT separation and precision | Verified | <ul><li>One wallet and one set of ledger accounts per customer and asset.</li><li>Asset units are immutable.</li><li>Exact decimal strings are tested up to 1e37.</li><li>A shared address is recorded per asset-network row, so a transfer's token contract identifies the asset.</li><li>A Phase 9 precision requirement is recorded below.</li></ul> |
| 10 | Frontend unavailable states | Verified | <ul><li>An unavailable API shows no balance.</li><li>Only a verified, receivable address is shown and copyable.</li><li>Pending, not-requested and unavailable states are distinct.</li><li>Unknown wallets return 404 without an API call.</li><li>Tests: `tests/web.test.mjs`, `tests/wallets.test.mjs`.</li></ul> |
| 11 | Phase 6 versus Phases 7–9 | Verified | <ul><li>Only IMP-P6-001–010 are delivered.</li><li>No transfer (Phase 7), adapter, worker or webhook (Phase 8), deposit intake or credit (Phase 9), or withdrawal (Phase 10) exists.</li><li>`POST …/deposit-addresses` creates a durable request that no process can fulfil until Phase 8; IMP-P9-001 completes provisioning.</li></ul> |
| 12 | Deposit-address QR (FR-WAL-010) | Not in Phase 6; deferred without a new task ID | <ul><li>FRS §11.1 places the QR in the deposit flow.</li><li>The roadmap delivers the deposit-address experience in Phase 9 (deposit address API) and Phase 17 §30.4 (Send and Receive).</li><li>No verified address can exist until a provider is enabled, so there is nothing to encode yet.</li></ul> |

### Phase 9 requirements recorded by this review

- **Attribution.** Identify deposits from address records of every status, matched on network, address and memo, together with the transfer's token contract. Never use the customer-visibility rule for attribution.
- **No automatic balance.** None of these may automatically create available balance (SEC-DEP-006, Test Strategy §18.4):
  - deposits to retired, suspended or rejected addresses;
  - deposits to wallets that cannot receive;
  - unsupported tokens or networks;
  - missing memos;
  - amounts below the confirmation policy.

  Route them to the review and suspense flow (ledger chart 6140, Unidentified Deposit Suspense) under Phase 9 and 16 controls.
- **Precision.** A contract's `token_decimals` may differ from the asset's accounting precision (6). Convert on-chain amounts exactly. An amount not representable at accounting precision goes to review and is never rounded.
- **QR code (FR-WAL-010).** Render the QR only for an ACTIVE, verified address, from the same API value used for copying. Encode only the address, plus the memo where the network requires one (FRS §21.1). Show the asset and network warning beside it (FRS §11.1).

## Migration modified in place

The fixes edit migration `202611261401` rather than adding a forward migration, for these reasons:
- PR #20 is unmerged.
- The migration exists only on this feature branch.
- It has been applied only to disposable databases that CI and `sqlx::test` create and drop for each run. No shared, persistent or `main`-derived database has recorded its checksum.

A forward migration would ship the defective function definitions for no benefit. Once the PR is merged, any further change must be a new forward-only migration.

## Residual risks and gates

- Revalidation protects provisioning once a Phase 8 worker exists. Today no adapter or worker runs, and `deypay_wallet_provisioner` is granted to no login.
- Revocations other than wallet status are not row-locked against the worker (see Concurrency).
- Precedence rule 1 makes a provider-wide `UNSUPPORTED` or non-operational row override narrower approvals. The default is recorded for owner confirmation in [DECISIONS.md](DECISIONS.md) (Phase 6 decision 6).
- Passing tests are engineering evidence only. They are not provider capability, regulatory, security or production approval. Nuvion and ImportaPay remain unverified `CANDIDATE` providers.
