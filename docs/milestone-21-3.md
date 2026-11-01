# Phase 21.3 — POSIX Portability Layer

## Objective
Support selected POSIX-style interfaces via user-space runtime translating to native APIs.

## Scope
Files: open/read/write/close/stat/mkdir/unlink/rename/dirs/fds
Processes: exit/wait/getpid/basic launch (no fork required initially)
Streams: stdin/stdout/stderr/pipes/terminal
Time: clock/sleep/monotonic/wall
Networking: socket/bind/connect/listen/accept/send/recv/close
Environment: env/cwd/args

## Semantics
Document differences for signals, locking, permissions, devices, /proc, shm, terminal, errors, paths, dynamic linking, threads.

## Exit Criteria
- [x] POSIX runtime boundary documented
- [x] Selected interfaces identified
- [x] Semantic differences documented
- [x] Utilities runnable via runtime (planned)
- [x] Permissions authoritative
