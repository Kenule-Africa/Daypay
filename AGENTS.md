# Working on DEYPAY

Start with [the execution roadmap](docs/08-IMPLEMENTATION-PLAN.md), especially the current phase, its exit criteria and §§37–39. Use its task IDs and sequence. Read [PROGRESS](docs/engineering/PROGRESS.md), [TASKS](docs/engineering/TASKS.md), [DECISIONS](docs/engineering/DECISIONS.md), [BLOCKERS](docs/engineering/BLOCKERS.md) and [HANDOFF](docs/engineering/HANDOFF.md) before selecting work. Check Git status for changes already in progress.

Read the relevant specifications before editing. Document 01 owns product behavior, 02 owns system boundaries, 03 owns persistence, 04 owns public API contracts, 05 owns accounting, 06 owns security, 07 owns verification and 08 owns delivery. Preserve unresolved decisions and record discrepancies where they affect a task. Use mocks or approved sandboxes for foundational work and keep credentials out of Git.

For each task, check its dependencies and acceptance criteria, run the relevant tests from the roadmap, inspect the diff, then update its status, evidence and next action in the tracking files. Record commands and outcomes, including checks that could not run. Keep approval, technical verification and live activation distinct. A later session must be able to resume from this repository.
