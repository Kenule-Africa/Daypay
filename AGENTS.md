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
- Follow the owner-approved GitHub Actions–first policy in [the test strategy](docs/07-TEST-STRATEGY.md): Claude, Codex and all future engineering agents must use GitHub Actions for expensive or broad tests wherever feasible, including full builds/suites, PostgreSQL/API integration, authentication regressions, financial invariants, frontend builds and broad static analysis. Do not substitute repeated full local runs when Actions covers the same revision.
- Keep local testing minimal and targeted: quick formatting checks, focused tests/debugging, manual UI review, private configuration checks, local hardware dependencies and explicitly authorized environment-specific acceptance are permitted where useful.
- For any claimed CI verification, identify the exact commit SHA and verify the completed GitHub Actions run for that commit, including successful required jobs and no silently skipped mandatory checks. The implementing agent owns monitoring CI on its branch, inspecting failed-job logs, fixing attributable failures, pushing corrections, and verifying the final commit. Report access limitations, failed or skipped checks, and only tests that actually ran; lack of access is not evidence of success.
- Check dependencies and acceptance criteria first; run targeted local checks as appropriate, then use Actions for broad or integration-boundary verification.
- Do not claim completion or passing tests without executing and recording the relevant checks. Distinguish type checking, compilation, runtime tests, simulations, provider approval, and live activation.
- Before completion, inspect the diff, update only directly related documentation and tracking evidence, and leave an actionable handoff. Keep final reports concise.

## Parallel coding-agent coordination

- One coding agent owns one active roadmap task. Codex and Claude use separate worktrees and feature branches:
  - Codex: `feat/codex-<task-id>-<description>`
  - Claude: `feat/claude-<task-id>-<description>`
- Each agent edits only inside its assigned worktree and remains within the files/modules recorded in [TASKS.md](docs/engineering/TASKS.md).
- Before implementation, use the task acceptance criteria and applicable specifications. Do not refactor unrelated code or silently change a shared migration, API contract, authentication/KYC boundary, compliance rule or financial invariant.
- Database migrations, shared API contracts and engineering tracker updates require explicit coordinator ownership. If an overlap is found, stop and report it; do not edit around it in parallel.
- Preserve existing tests and add focused regression coverage. Run only the test database/worktree scope assigned to the task. Prefer the existing disposable PostgreSQL/SQLx pattern, and serialize or isolate local SQLx test runs if their generated database names or environment variables could overlap.
- Commit and push only to the assigned feature branch. Never push directly to `main`, merge a pull request, force-push, change repository settings or bypass reviews.
- End each implementation with a pull request targeting `main`. Include the task ID, summary, changed files, actual test outcomes, security/regression considerations and remaining blockers.
- CI must pass before integration. The repository owner alone approves and merges PRs.
- Before new work, branch from current `main` after the owner's prior merge acceptance. Incorporate the latest accepted `main` changes into a feature branch when dependencies require it; use a normal merge or rebase only on that agent's branch.
- Copilot may coordinate, prepare review findings and record accepted evidence. It does not approve or merge implementation assumptions.
