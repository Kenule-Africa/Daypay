# Blockers and release gates

| Affected work | Condition | Owner and clearance evidence |
|---|---|---|
| IMP-P1-002 / IMP-P1-003 native verification | Rust 1.100.0 is installed for x86_64-pc-windows-msvc. No usable MSVC installation was reported by vswhere. The shell resolves link.exe to the Git utility and Cargo cannot link the test executables. | Windows development environment owner. Verify the MSVC C++ build tools and Windows SDK in a Developer PowerShell, then build and test the workspace. |
| IMP-P1-002 / IMP-P1-003 lint verification | Windows Application Control blocked cargo-fmt.exe and clippy-driver.exe (OS error 4551). Direct rustfmt checks passed for all six source files. | Development environment / IT owner. Supply a policy-approved toolchain or runner for the roadmap lint commands. |
| IMP-P1-005 and database integration tests | No PostgreSQL client on PATH or installation in the standard location. Connection to 127.0.0.1:5432 was refused. | Local development environment owner. Supply an accessible local instance, verify server version and run an authenticated development query. Keep credentials in the local environment. |
| IMP-P1-006 | Docker CLI unavailable on this PATH. Local PostgreSQL is the initial development target. | Engineering. Check Docker availability when the roadmap reaches container validation. |
| Phase 0 approvals | Product scope and operating model are documented. Formal approval records, final financial mappings and named reviewers are pending. | Product, Finance, Security and Legal as applicable. Attach dated approval references before closing the corresponding review. |
| Live financial execution | Account-specific provider evidence, applicable operating approvals and reconciliation responsibilities remain outstanding. | Owners in the [decision log](DECISIONS.md) and [provider register](PROVIDER-CAPABILITIES.md). Simulated engineering may proceed. |
