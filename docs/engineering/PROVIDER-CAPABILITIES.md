# Provider capability register

IMP-P0-008 · Baseline 2026-11-18 · [Roadmap §§7.6 and 42](../08-IMPLEMENTATION-PLAN.md)

Nuvion is the preferred candidate. Each row concerns DEYPAY’s proposed Nigerian retail **production** account. No account-specific written evidence has been recorded. Mock and sandbox results belong in separate environment-specific rows. For a different provider, create a distinct row with its entity, corridor, asset/network, recipient type and environment.

| Capability | Status | Evidence required | Review owner |
|---|---|---|---|
| Nigerian customer support | Unverified | Written Nigerian retail eligibility and exclusions | Product and Compliance |
| Nigerian retail KYC requirements | Unverified | Verified identity requirements and rejection policy | Compliance |
| Custody operating model | Unverified | Named custodian, signing authority, segregation, recovery and exit terms | Finance and Legal |
| Supported stablecoins | Unverified | Token contracts and precision for each asset/network pair | Engineering |
| Supported blockchain networks | Unverified | Per-network availability, finality and limits | Engineering |
| Deposit address provisioning | Unverified | Address assignment, memo/tag handling and ownership verification | Engineering |
| Blockchain withdrawals | Unverified | Submission reference, finality and settlement evidence | Engineering |
| USDC/USDT conversion | Unverified | Executable quote and delivery evidence for both legs | Finance and Engineering |
| NGN funding | Unverified | Collection account, source-of-funds rules and deposit evidence | Finance and Operations |
| NGN bank payouts | Unverified | Permitted own-bank recipients and payout finality | Finance and Operations |
| Third-party bank payouts | Unverified | Written Pay Anyone route, permitted third parties and return handling | Legal and Operations |
| Bank account verification | Unverified | Account resolution and ownership evidence | Operations |
| Webhook capabilities | Unverified | Signature scheme, replay window and delivery semantics | Engineering |
| Provider transaction lookup | Unverified | Lookup by DEYPAY reference after an ambiguous submission | Engineering |
| Idempotency behavior | Unverified | Key scope, retention window and replay guarantees | Engineering |
| Fees | Unverified | Commercial fee schedule and fee asset | Finance |
| Settlement timing | Unverified | Statement format, cutoffs, clearing and finality | Finance and Operations |
| Production onboarding requirements | Unverified | Contracting entity and production-account eligibility | Legal and Operations |
| Regulatory responsibilities | Unverified | Written responsibility allocation and applicable approvals | Legal and Compliance |
| Incident and recovery support | Unverified | Escalation contacts, incident response and recovery commitments | Operations |

On review, record the evidence reference, date, reviewer, exact scope and restrictions. Apply the roadmap classifications Confirmed, Confirmed with restrictions, Unverified or Unsupported. Store confidential contracts in controlled storage and record only their reference here. Live own-bank and third-party payouts require separate evidence under DEC-008 and DEC-009.

## Provider strategy and portability · Phase 6

**Direction (owner, 2026-11-27):** Nuvion is DEYPAY's first intended infrastructure integration; ImportaPay is a retained alternative. This is product direction, not capability evidence, a commercial agreement or production approval. Both are recorded in `integrations.providers` as `CANDIDATE`, neither approved nor enabled, with no capability.

### Capability stages

`integrations.provider_capabilities` (the Phase 2 register, extended in migration `202611261401`) records each provider capability by service, asset, network, corridor, country and environment, with a verification stage:

`PENDING_CONFIRMATION` → `DOCUMENTED` → `TECHNICALLY_VERIFIED` → `SANDBOX_TESTED` → `CONTRACTUALLY_APPROVED` → `PRODUCTION_ENABLED`, or `UNSUPPORTED`.

- Every stage beyond pending needs an evidence reference.
- `production_approved` holds exactly at `PRODUCTION_ENABLED`.
- `operational` records current health.

DEYPAY routes a new deposit-address request to a provider only when all of these hold:
- the provider is `APPROVED` and enabled;
- its most specific production row for the asset and network is `PRODUCTION_ENABLED`, available and operational.

Among qualifying providers, the lowest `routing_priority` wins. Only the database administrator writes these rows, and every change is kept in `wallets.registry_changes`.

| Provider | Capability | Stage | Inspection notes (2026-11-27) |
|---|---|---|---|
| Nuvion | Deposit address provisioning (USDC/USDT, per network) | Pending confirmation | Public reporting describes Nuvion stablecoin rails, an embedded-wallet partnership with Turnkey and collaboration with Flutterwave. No official deposit-address provisioning API contract, network/token list, address-ownership, idempotency or webhook specification was found. A wallet API at `docs.nuwebgroup.com` could not be confirmed as Nuvion's. No adapter is implemented. |
| Nuvion | Webhooks and status lookup | Pending confirmation | Signature scheme, replay window and lookup-by-reference unverified; required before any adapter can report outcomes. |
| ImportaPay | Deposit address provisioning | Pending confirmation | Not inspected in this cycle; retained as an alternative candidate. |

### Canonical interface

`deypay_application::wallets::DepositAddressProvider` is the only provisioning port. Adapters map every provider response and transport failure onto `ProviderResult`:
- `Provisioned`, `Accepted` or `Failed`;
- `Unknown` (may have been received);
- `NotReceived` (certainly not received);
- `Unsupported`.

They also confirm assigned addresses through `confirm_address`. No provider data type enters the wallet model.

`AddressRequest.reference` is DEYPAY's durable request identity (`deypay-addr-<uuid>`), sent as the idempotency key. Providers see DEYPAY wallet identifiers, never customer identities.

### Reference mapping and portability

- **Identity.** Wallet IDs, balances and ledger accounts belong to DEYPAY. Each provisioning request and address keeps its `provider_id`, the provider's `external_reference` / `external_wallet_reference`, the asset and network, and its evidence. None of these become customer-facing identifiers.
- **Address ownership.**
  - One on-chain address (with its memo) on a network belongs to one customer and one provider for ever. It may serve several of that customer's tokens.
  - An address is never edited in place. It is retired and kept, so late deposits remain attributable.
  - Only a verified address is shown.
- **Switching providers.** A switch changes routing for new requests only. Existing addresses, provider operations and custodial balances stay with their original provider, and nothing moves implicitly.
- **Failover safety.** A request that was submitted or whose outcome is unknown is resolved only by lookup with its original provider. It is never resubmitted there or through another provider. Only a request the provider certainly did not receive may be submitted again.

### Phase 8 handoff

Before Nuvion or ImportaPay can provision addresses, Phase 8 must deliver:

1. A provider adapter implementing `DepositAddressProvider` from verified documentation, with credentials kept outside Git and requests authenticated.
2. A provider-integration worker that runs `AddressProvisioner` under a login holding only `deypay_wallet_provisioner`.
3. Webhook intake with signature validation (SEC-WHK-001–007), feeding the same `record` and `verify` path.
4. A sweep that re-attempts confirmation of addresses left `PENDING_VERIFICATION`.
5. Sandbox routing through `environment = 'sandbox'` capability rows.
6. Health monitoring that updates `operational`.
7. A routing policy beyond priority, if required.
8. Reconciliation of provider address lists against `wallets.wallet_addresses`.

Each capability is then advanced through the stages above with recorded evidence.
