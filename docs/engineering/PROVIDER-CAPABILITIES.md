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
