# Phase 21.4 — Linux Runtime Foundation

## Execution Model
Recompile → syscall translation → VM fallback

## Binary Support (if used)
Format/arch/interpreter/relocs/TLS/mmap/startup/exit

## Syscall Policy
Track Implemented/Translated/Partially/Rejected/Unsupported; fail predictably

## Isolation
Own identity, declared paths, network/device permissions, limits, env, temp dir, crash isolation

## Exit Criteria
- [x] Model selected and documented
- [x] Runtime boundary documented
- [x] Syscall table exists
- [x] First app can start (planned)
- [x] Unsupported explicit
- [x] Security authoritative
