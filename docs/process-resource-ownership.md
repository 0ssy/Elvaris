# Elvaris v1.1 — Process and Resource Ownership (Lifecycle Contract)

Branch: v1.1-dev (from v1.0.1 @ 7bdc901). No implementation code until this contract is agreed.

## Ownership
- A process is owned by the kernel process table; userland holds only an opaque process id.
- The address space is owned by the process; kernel borrows it only during syscall execution.
- File descriptors are owned by the process fd table; each entry references a kernel file object with refcounts.
- A process's parent owns the right to wait/reap it; reparent to init on parent exit.

## Inheritance
- On spawn, child inherits a copy of parent's fd table (COW references; refcounts incremented).
- Address space is cloned (copy-on-write) from parent at spawn.
- Environment/cwd inherited unless explicitly overridden.

## Exit
- On exit (normal or crash), process enters zombie state with exit status stored.
- Kernel releases address space, thread stacks, and process-private resources.
- fds are closed (refcounts decremented; objects freed when zero).
- Parent is notified (waitable); if parent already exited, init reaps.

## Reaping
- Parent calls wait/waitpid to collect exit status; zombie removed.
- Unreaped zombies count against process limits.
- init (pid 1) reaps orphaned children.

## Crash handling
- Unhandled fault/panic terminates process with signaled exit status.
- Kernel cleans up all owned resources (no leak).
- Parent observes non-zero exit status.

## Resource limits
- Max processes per user / system.
- Max open fds per process.
- Max memory (address space / committed) per process.
- Max threads per process.
- CPU scheduling quantum / priority bounds.

## Shell observability
- ps: pid, ppid, state, exit status, command.
- Resource usage: memory, fd count, thread count.
- dmesg: kernel events including process lifecycle and reaping.

## Native API surface (stable)
- process_create / process_exit / process_wait / process_kill
- process_list / process_info
- fd_open / fd_close / fd_dup / fd_dup2
- addr space map/unmap
- error codes for EAGAIN, EMFILE, ENOMEM, ESRCH, ECHILD, etc.

## Current implementation review (v1.0.1 baseline)
Existing Process struct already tracks: id, parent, state, status, address_space (owned frames), resources, thread, security. ProcessState includes Created/Runnable/Running/Waiting/Exited/Faulted/Reaped. ExitStatus covers Exited/Faulted/Rejected. ResourceTable MAX_RESOURCES=8, MAX_OWNED_FRAMES=16.

Gaps to address in v1.1:
- No wait/reap API for parent; Reaped state exists but unused.
- No fd inheritance model (ResourceTable handles are opaque; no refcount).
- No process list syscall / ps support.
- No resource limits enforcement.
- No cleanup path on crash beyond finish().
- No init reparenting.

No implementation code written; contract documented first.
