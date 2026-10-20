# Milestone 9: User Mode

## Objective

Start one isolated user-space program at a lower privilege level with its own address space, controlled syscall path, and clean exit.

## Scope

- User/kernel address-space layout and permissions
- Single user program load and execution
- User stack creation
- Privilege transition to ring 3
- `write` and `exit` system calls
- User pointer validation
- User fault isolation

## Kernel/User Boundary

Kernel virtual range stays supervisor-only; user range is user-accessible. Kernel mappings may remain visible but non-user. User-mode code cannot read or write kernel-only pages.

## User Address Space

Define user and kernel virtual ranges, page permissions, address-space ownership, page-table ownership, and teardown behavior. User pages are marked user-accessible; kernel pages remain supervisor-only.

## User Program Format

Use the simplest reliably testable format (minimal custom or fixed in-memory image); ELF as long-term direction. No dynamic linking, shared libraries, or relocations in first program.

## User Stack

Allocate a user stack in the user address space with proper permissions and initial stack pointer.

## Privilege Transition

Enter user mode via interrupt-return frame with user CS/DS/SS, initial RIP and RSP. Define fault handling for user code.

## System-Call Entry

Controlled syscall entry path with validated syscall number and arguments.

## Initial System Calls

- `write`: console output with validated user pointer, length, address range, page accessibility, and output size.
- `exit`: terminate user program without kernel crash.

## Process and Thread Ownership

Single user process/thread owned by kernel; teardown on exit or fault.

## Memory Permissions

User pages user-accessible; kernel pages supervisor-only. Enforce no user access to kernel-only pages.

## Failure Behavior

Invalid instruction, page fault, GPF, invalid syscall, invalid user pointer, stack overflow, unexpected return: terminate user program and report via serial. Not a kernel panic.

## Testing Strategy

Unit tests for pointer validation and address-space helpers; QEMU integration test running user program end-to-end.

## Diagnostics

Serial reporting of user program start, syscall activity, exit, and fault reasons.

## Out of Scope

Multiple processes, fork, exec, filesystems, dynamic linking, shared libs, signals, user threads, sync primitives, networking, shell, POSIX, ARM64, copy-on-write, demand paging.

## Exit Criteria

- [ ] User/kernel address-space layout documented
- [ ] User pages marked user-accessible
- [ ] Kernel pages remain supervisor-only
- [ ] User stack created
- [ ] User program loaded
- [ ] CPU enters user mode
- [ ] User code executes
- [ ] Syscall entry path works
- [ ] User pointers validated
- [ ] User output reaches serial console
- [ ] User program can exit
- [ ] User faults isolated from kernel
- [ ] Invalid syscalls rejected
- [ ] Unit tests pass
- [ ] User-mode QEMU integration test passes
- [ ] Existing scheduler and memory tests pass
- [ ] Limitations documented
