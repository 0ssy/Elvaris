# Phase 21.2 — Portable C and Rust Application Support

## Objective
Make ordinary portable applications run natively on Elvaris without foreign OS runtime.

## C/Rust Runtime Support
- C: startup/exit, malloc/free, file ops, dirs, stdio, time, basic threads/sockets, env vars
- Rust: no_std/alloc/std, panic behavior, threads, fs/net/time, async limitations documented

## Portability Guidance
- Paths/env/permissions/signals/process/dynamic libs/threads/terminal translated appropriately

## Reference Apps
hello, file-reader/writer, directory-lister, process-status, network-client/server, graphics-test, audio-test, permission-test

## Exit Criteria
- [ ] C/Rust portability documented
- [ ] Runtime libraries work
- [ ] Representative apps run natively
- [ ] Porting guide exists
- [ ] Native builds simpler than compatibility
