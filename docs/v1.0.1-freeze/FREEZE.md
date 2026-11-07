# Elvaris v1.0.1 — Freeze Record

Status: FROZEN (immutable). Do not modify tagged commit 7bdc901.

## Tag
- v1.0.1 -> commit 7bdc901a1682facf3f974101906e2077f2604151
- Tagged: 2026-11-06

## Acceptance results
- Clean-start acceptance validation passed (exact tag 6344229, detached worktree; temp worktrees removed after run).
- v1.0.0 clean-start acceptance validation completed.
- Release scope frozen per Milestone 22 (22.0-22.15).

## Scope
Elvaris v1.0.1 is complete within its declared scope. No further v1.0 work.

## Build (clean start)
See RUN_ELVARIS.md. Summary:
1. Install toolchain (rust-toolchain.toml).
2. `$env:PATH = "C:\Users\josep\.cargo\bin;$env:PATH"`
3. `cargo build -p os`
4. Run QEMU with OVMF firmware per RUN_ELVARIS.md.

## Warnings
Warnings are NOT fixed in v1.0.1. Tracked in docs/warnings-backlog.md for v1.1.

## Forward
All future work targets v1.1 — "Native Userland and Persistent Storage".
