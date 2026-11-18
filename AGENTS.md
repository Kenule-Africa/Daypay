# DEYPAY agent workflow

## Session startup

1. Read this file and [the handoff](docs/engineering/HANDOFF.md).
2. Check Git status and identify the active task ID.
3. Read only that task in [TASKS.md](docs/engineering/TASKS.md), its phase and exit criteria in [the implementation roadmap](docs/08-IMPLEMENTATION-PLAN.md), and the relevant entries in [CODEBASE-MAP.md](docs/engineering/CODEBASE-MAP.md).
4. Read the affected source files and the applicable specification sections before editing.
5. Consult [DECISIONS.md](docs/engineering/DECISIONS.md), [BLOCKERS.md](docs/engineering/BLOCKERS.md), and adjacent modules when dependencies require them.

Do not load all specifications or the whole repository by default. Use targeted searches and exact paths, and expand context whenever dependencies, ambiguity, security, or financial correctness require it.

## Authority and reliability

- Evidence is required from the authoritative specification and actual source code; handoffs, maps, prior chats, and assumptions are navigation aids, not proof.
- Use FRS for product behavior, architecture for boundaries, database design for persistence, API specification for public contracts, ledger specification for accounting, security specification for security/compliance, test strategy for verification, and the implementation roadmap for sequencing.
- Never invent routes, fields, states, schema, provider capabilities, authentication, security policy, financial rules, or approvals. Record significant unspecified choices through the existing decision process.
- If specifications conflict or evidence is missing, investigate, report the discrepancy or blocker, and do only safe independent work.
- For deposits, withdrawals, conversions, transfers, balances, fees, reservations, NGN payments, or reconciliation, inspect every affected ledger, database, API, security, functional-requirement, and test-strategy section plus the relevant source interfaces.

## Implementation and verification

- Work only on the assigned task and preserve existing decisions, task IDs, statuses, and unresolved gates.
- Make small, reviewable changes using existing patterns. Keep credentials and secrets out of Git.
- Check dependencies and acceptance criteria first; run focused tests, then broader checks at integration boundaries.
- Do not claim completion or passing tests without executing and recording the relevant checks. Distinguish type checking, compilation, runtime tests, simulations, provider approval, and live activation.
- Before completion, inspect the diff, update only directly related documentation and tracking evidence, and leave an actionable handoff. Keep final reports concise.
