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

The kernel and the user program share one page-table hierarchy for now. The boundary is enforced by two different mechanisms:

- **Leaf entries.** Every kernel mapping is supervisor-only: its page-table entry has no user bit, so the processor refuses a user-mode access to it, whatever the level above it says.
- **Upper-level entries.** The tables on the way to a user page carry the user bit. The processor checks the user bit at *every* level of the walk for a user-mode access, so a table without it makes the pages below it unreachable from ring 3 even when the page itself is accessible.

The kernel therefore widens the flags of an existing table when it maps a user page: `Mapper::next_table` adds the user bit to the entry of the table it walks through. The frame of the table is never lost, and the no-execute bit is removed from the table entries, because a no-execute bit on an intermediate entry makes every page below it non-executable.

The kernel heap (`0xFFFF_A000_0000_0000`), the physical-memory direct map (`0xFFFF_8000_0000_0000`), the memory mapped device registers (`0xFEE0_0000`) and the kernel image itself stay supervisor-only. A user program that names any of them gets a page fault, and the fault terminates the program, not the kernel.

## User Address Space

The user address space is a fixed window in the low half of the address space, far away from the kernel heap, from the direct map and from the device registers:

```text
 0x0000_0000_0100_0000  user code      1 page   read + execute
 0x0000_0000_0100_1000  user data      1 page   read + write, no execute
 0x0000_0000_0100_2000  user stack     4 pages  read + write, no execute
 0x0000_0000_0100_6000  end of the user address space
```

| Property | Value |
|----------|-------|
| Address space ownership | The kernel owns the window for the whole lifetime of the program. The program never owns memory the kernel cannot take back. |
| Page-table ownership | The kernel creates every table and every page. The user program cannot map, unmap or change a permission. |
| Code page | Present, user-accessible, executable, **read-only** once the program is loaded. The program cannot rewrite the code the processor runs. |
| Data page | Present, user-accessible, writable, no-execute. |
| Stack pages | Present, user-accessible, writable, no-execute. |
| Teardown | The pages stay mapped after the program ends. The PMM cannot reclaim frames, so unmapping would not give the memory back; the window is reused by the next program. |

The code page is mapped writable while the kernel copies the image into it, and is made read-only with `Mapper::update_flags` right afterwards. The first page of the address space is never mapped, so a null pointer in user code faults instead of reaching the kernel.

## User Program Format

The first user program is a **fixed in-memory image**: a block of machine code the kernel assembles itself in `Kernel/src/user/program.rs` and copies into the user code page. There is no executable format, no loader and no relocation: the image is loaded at a fixed address, and the entry convention is fixed.

- The image is a single contiguous block; the kernel copies it with one `copy_nonoverlapping`.
- The kernel hands the program its arguments in registers: `RDI` points at the message the program is expected to write, `RSI` holds the length of that message.
- The program is entered at the base of the user code page.

The image holds three programs: the user program proper, and two probes that do something the kernel has to refuse (see *Failure Behavior*). ELF is the long-term direction; nothing in this milestone depends on the format beyond "a block of bytes at a known address".

## User Stack

The user stack is four pages at `0x100_2000`, mapped writable and non-executable. The stack grows downwards, and the program starts with `RSP = 0x100_6000`, the first address above the stack region.

The stack belongs to the user program, but the kernel owns it: the pages come from the PMM through the mapper, and the kernel never writes to them.

Events that arrive from user mode do **not** use the user stack. The task state segment holds the address of a separate 64 KiB interrupt stack in the kernel heap, and the processor switches to that stack whenever an interrupt or an exception crosses the privilege level. The user program never touches it.

## Privilege Transition

The kernel enters user mode with `iretq`. The instruction pops the instruction pointer, the code segment, the flags, the stack pointer and the stack segment of the interrupted code, so the frame the kernel builds on its own stack is exactly the frame `iretq` expects:

```text
 high  ┌───────────────┐
       │ ss            │  user stack segment
       │ rsp           │  user stack pointer
       │ rflags        │  user flags
       │ cs            │  user code segment
 low   │ rip           │  first instruction of the user program
       └───────────────┘
```

- `CS` is the user code segment with a requested privilege level of three (`0x1B`), `SS` is the user data segment with a requested privilege level of three (`0x23`).
- `RFLAGS` is `0x202`: the interrupt flag is set, so a user program may be interrupted, and bit one reads as one.
- `DS` and `ES` are loaded with the user data segment before the transition, so the program does not have to do it before it touches memory.
- The transition itself is in `Kernel/src/user/session.rs`; the assembly stub also records where the kernel has to resume, so the kernel does not have to guess.

Fault handling for user code is described in *Failure Behavior*.

## System-Call Entry

A user program reaches the kernel through one instruction: `int 0x80`. The interrupt descriptor table entry of that vector is the only entry whose requested privilege level is three; every other entry belongs to ring zero. A user program therefore cannot raise any other interrupt, and the kernel stays in control of the entry points.

The registers follow the System V convention:

| Register | Meaning |
|----------|---------|
| `RAX` | the number of the system call |
| `RDI` | the first argument |
| `RSI` | the second argument |
| `RDX` | the third argument |
| `RAX` | the return value |

The arguments are untrusted. Every pointer a program passes is validated against the user address space and against the page tables before the kernel dereferences it; see *Initial System Calls*.

## Initial System Calls

| Number | Call | Signature |
|--------|------|-----------|
| 1 | `write` | `write(fd, buffer, length)` |
| 2 | `exit` | `exit(code)` |

**`write`** writes to the serial console and returns the number of bytes written. The kernel refuses the call when

- the file descriptor is not the console (`1`),
- the length is longer than the limit of 64 KiB,
- the buffer is not a canonical address,
- the buffer or the range it describes leaves the user address space,
- the addition of the address and the length overflows,
- a page of the range is not mapped,
- a page of the range is not accessible from user mode.

The check of the pages walks the page tables of the active address space (`Kernel/src/vmm/translate.rs`), because the mapper owns the physical-memory allocator and cannot be borrowed from an interrupt handler.

**`exit`** terminates the user program with the given exit code. The kernel does not return to the program: it restores the kernel stack, resumes where it left user mode, and reports the outcome.

## Process and Thread Ownership

A single user process with a single thread, owned by the kernel for its whole lifetime: the kernel creates the address space, loads the program, enters ring 3, and takes the program down again when it ends — either through `exit` or through a fault. The program is never scheduled, and there is no second program.

The session state (`Kernel/src/user/session.rs`) records the resume stack pointer, the resume address, the outcome, the exit code and the number of system calls.

## Memory Permissions

User pages are user-accessible; kernel pages remain supervisor-only. The rules the kernel enforces:

- The user bit is set on the code page, the data page and the stack pages, and on every table on the way to them.
- The kernel heap, the direct map, the device registers and the kernel image have no user bit on their leaf entries, so a user access to them faults.
- The code page is read-only: the user program cannot change the code the processor runs.
- The data page and the stack are writable but not executable: data must never run as code.
- The page tables themselves belong to the kernel. A user program cannot map, unmap or change a permission.

## Failure Behavior

Every failure below terminates the **user program** and is reported on the serial console. None of them is a kernel panic.

| Event | Kernel reaction |
|-------|-----------------|
| Invalid instruction | The program is terminated with the vector of the exception (6). |
| Page fault | The program is terminated with the vector of the exception (14); the address, the error code, `RIP` and `RSP` are reported. |
| General protection fault | The program is terminated with the vector of the exception (13). |
| Invalid system call | The number is reported, and the program is terminated as *rejected*. |
| Invalid user pointer | The reason is reported (not canonical, outside the user space, overflow, unmapped, not user-accessible, not writable), and the program is terminated as *rejected*. |
| Stack overflow | The stack pages end at the end of the user window, so a program that grows the stack past it faults and is terminated like any other page fault. |
| Unexpected return | The program ends with `exit`, or the kernel terminates it. A program that somehow returns from a system call the kernel ended never resumes: the resume address is cleared when the session ends. |
| Exception from user mode outside a session | The kernel reports it as an unexpected event and stops the machine, because the kernel itself switched the privilege level without preparing for it. |

The kernel survives all of them: the probes of the self test (see *Testing Strategy*) fault and raise an unknown system call on every boot.

## Testing Strategy

**Host unit tests** (`cargo test -p kernel --lib`, 94 tests) cover the pure parts of the subsystem:

- the layout of the user address space and the permissions of its regions,
- the validation of the ranges a user program hands to the kernel (canonical address, inside the window, no overflow, zero length),
- the interpretation of the permission bits of a page-table entry,
- the privilege level of the user segments and the initial flags of the program,
- the shape of the user image and the entry addresses of the program and of the probes,
- the numbering of the system calls.

**QEMU integration test** (`cargo test -p kernel --test user`) boots the kernel and checks that

- the kernel reports the layout of the user address space and enters ring 3,
- the output of the user program reaches the serial console,
- the program exits with the code it asked for,
- the kernel counts the system calls,
- the fault probe is taken down with a page fault and the kernel survives,
- the unknown-system-call probe is rejected and the kernel survives,
- the kernel reports neither an exception nor a panic.

The self test of the user mode runs the two probes on every boot, in the same way the heap runs its own self test. The probes live in the user image next to the user program, so one copy of the code page holds all three.

## Diagnostics

The serial console reports

- the layout of the user address space and the address of the interrupt stack,
- the raw page-table entries of the first page of every region, so a permission that the processor refuses can be traced to the level that carries it,
- the entry address and the stack pointer of the user program,
- the output of the user program itself, through the `write` system call,
- the exit code of the program, the outcome of the session and the number of system calls,
- the reason a program was taken down: the vector, the error code, `RIP`, `RSP` and the faulting address of a page fault, the number of an unknown system call, or the reason a pointer was refused,
- the result of the self test.

## Out of Scope

Multiple processes, fork, exec, filesystems, dynamic linking, shared libs, signals, user threads, sync primitives, networking, shell, POSIX, ARM64, copy-on-write, demand paging.

## Limitations

- **One address space.** The user program shares the page-table hierarchy with the kernel, and the user window lives in the PML4 entry the bootloader used for the kernel image. The kernel widens the flags of the tables on the way to the user pages; the pages themselves keep the permissions they need.
- **No teardown.** The pages of a finished program stay mapped, because the PMM cannot reclaim frames. The window is reused by the next program.
- **No ELF.** The program is a fixed image at a fixed address, with a fixed entry convention.
- **Two system calls.** `write` writes to the console, `exit` ends the program. There are no files, no other descriptors and no other processes.
- **The kernel stack of a user program is not a kernel stack.** The processor switches to the privilege stack of the task state segment for every event from user mode; that stack is shared by every program the kernel runs.
- **The address space is not isolated.** The user window is a range in the kernel's own address space, not a separate one. A user program cannot reach the kernel, but the kernel does not have to switch page tables to run one.

## Exit Criteria

- [x] User/kernel address-space layout documented
- [x] User pages marked user-accessible
- [x] Kernel pages remain supervisor-only
- [x] User stack created
- [x] User program loaded
- [x] CPU enters user mode
- [x] User code executes
- [x] Syscall entry path works
- [x] User pointers validated
- [x] User output reaches serial console
- [x] User program can exit
- [x] User faults isolated from kernel
- [x] Invalid syscalls rejected
- [x] Unit tests pass
- [x] User-mode QEMU integration test passes
- [x] Existing scheduler and memory tests pass
- [x] Limitations documented
