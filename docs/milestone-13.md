# Milestone 13 — Process and Resource Maturity

## Goal
Make repeated process creation/exit safe by reclaiming user memory, page tables, stacks, and handles; add parent waits and reaps.

## Completed
- PMM bitmap allocator with release/double-release protection
- Process states extended (Waiting, Reaped)
- Process table helpers (find/find_mut/reap)
- Syscall numbers for wait/get_pid/get_ppid added
- Cleanup hooks for exit/fault paths
- Address space cleanup with frame tracking
- Deferred stack reclamation support
- Scheduler termination/reap support
- Resource table tracking and cleanup
- Process limits (MAX_PROCESSES) enforced
- Wait/reap behavior implemented
- Stress test added for repeated process lifecycle

## Resource Ownership
- User frames tracked in AddressSpace for cleanup
- Mappings cleaned up on exit/fault
- Kernel mappings preserved
- File handles cleared on process cleanup
- PID reuse only after reap

## Validation
- cargo test passes (all unit/integration tests green)
- Repeated process lifecycle tested
