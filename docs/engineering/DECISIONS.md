# Engineering decisions

[Roadmap §51](../08-IMPLEMENTATION-PLAN.md) owns DEC-001 through DEC-015. Use its IDs when recording evidence here. The live route, fees, custody and refund terms remain open. No external approvals are recorded in this repository.

| Topic | Current basis | Engineering consequence |
|---|---|---|
| Implementation authority | Product owner directed implementation using document 08 on 2026-11-18 | Use the existing tasks and phase exit criteria. Formal baseline and production sign-offs have separate owners. |
| Scope and NGN model | FRS §§6, 13–17 and architecture §3 | Customer wallets are USDC and USDT. NGN supports funding, conversion and settlement. Pay Anyone is required and can use simulated execution. |
| Custody review | Architecture §§13, 46.1 and security §16 | Third-party custody is the design assumption. The legal custodian, key control, safeguarding and recovery evidence remain outstanding under DEC-004. |
| Accounting review | Ledger §§10–11, 19, 25, 43 | Use available and reserved liability accounts. A confirmed conversion replaces the stablecoin reservation with an NGN obligation. Finance approval and DEC-015 remain open. |
| API review | API §§11, 20, 38 and security SEC-TXN-001–007 | Bind authorization to immutable terms, accept a financial request once and map financial states to public statuses. |
| Local development | Product owner specified local PostgreSQL initially | Inspect and validate a local service before database work. Use synthetic fixtures and the Rust workspace in the meantime. |

Provider verification is tracked in the [capability register](PROVIDER-CAPABILITIES.md). Legal dependencies identified for IMP-P0-009 include operating entity and permissions, custody and safeguarding, AML reporting, customer data handling, complaints, and permitted funding and settlement recipients. Legal and Compliance own DEC-004 and DEC-006–009 with Finance, Operations and the contracting partners as applicable. See FRS §45 and security §44.
