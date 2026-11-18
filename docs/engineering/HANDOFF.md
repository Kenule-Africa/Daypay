# Engineering handoff

Read [PROGRESS.md](PROGRESS.md) and [TASKS.md](TASKS.md) against the current Git tree. The eight specifications are committed at `e96db69`. Use [BLOCKERS.md](BLOCKERS.md) for local tooling and production gates.

**Next task:** Finish native verification for IMP-P1-002 and IMP-P1-003. Their manifests and source roots already exist. Use a Windows Developer PowerShell with the MSVC linker and Windows SDK, or another approved Rust build runner. Run from `backend/`:

```text
cargo metadata --format-version 1 --locked --offline
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo build --workspace --locked --offline
cargo test --workspace --locked --offline
```

Record results in PROGRESS.md and update TASKS.md. Type checking, the four library builds and direct formatting passed at kickoff. The next Phase 1 task in the roadmap after crate verification is IMP-P1-004, initializing the Next.js frontend. Local PostgreSQL setup is IMP-P1-005. Phase 0 approvals can proceed in parallel.

From the repository root, run `node --test scripts/engineering-tracking.test.mjs` after changes to the backlog or provider register. The kickoff files were left uncommitted on main. The user's untracked `prompt.md` was already present. The baseline specifications are tracked at the commit above.
