# Milestone 7: Thread Context Switching

## Objective

Prove that the kernel can safely pause one kernel thread and resume another. This milestone establishes the foundation for kernel threads and scheduling by implementing cooperative context switching before introducing preemption.

## Scope

- Kernel thread structure and lifecycle
- Fixed-size kernel stacks allocated from the kernel heap/PMM
- Saved CPU context definition
- Architecture-specific context switch routine
- Cooperative yield mechanism
- Thread creation and termination
- Basic runnable thread selection
- Context switch diagnostics
- Host unit tests and QEMU integration tests

## Thread Model

For now, threads are entirely in kernel space. Each kernel thread owns:

- CPU register context
- Kernel stack (fixed-size)
- Thread identifier (TID)
- Thread state
- Entry function pointer
- Argument/data pointer
- Optional name for diagnostics

## Saved CPU Context

Context switching must preserve state across threads. Registers are categorized as follows:

### Caller-saved registers
Handled by the calling convention (not preserved by the switch routine):
- RAX, RCX, RDX, RSI, RDI, R8, R9, R10, R11

### Callee-saved registers
Must be preserved by the context switch routine:
- RBX, RBP, R12, R13, R14, R15

### Control state
Must be preserved:
- RSP (stack pointer)
- RIP (instruction pointer) - stored via call/ret or explicit save

### Optional state (deferred)
Not saved in Milestone 7:
- Floating-point (x87)
- SSE/AVX registers (XMM/YMM/ZMM)
- MXCSR
- FS/GS base
- Other extended CPU state

These will be added when needed. The implementation must document what is deferred.

## Stack Model

### Allocation
- Stacks allocated via kernel heap (which uses VM mappings backed by PMM)
- Fixed-size initially (e.g., 16 KiB or 64 KiB)
- Page-aligned for safety and future guard page support

### Layout
- Stack grows downward on x86_64
- Newly created thread stacks must have a valid initial frame that makes the thread appear as if it was called normally
- Initial frame must set up: entry function, argument, return address (to thread exit/cleanup)

### Behavior
- A kernel thread that returns from its entry function must not return to an invalid address
- Termination path must clean up or transition thread to Terminated state
- Guard pages are planned for later milestones but not required now

## Context-Switch Contract

The low-level context switch operation:

**Input:**
- Pointer to current thread's context (or structure containing saved registers/stack pointer)
- Pointer to next thread's context

**Operation:**
1. Save current thread's callee-saved registers and control state
2. Switch stack pointer from current to next
3. Restore next thread's callee-saved registers and control state
4. Resume execution in next thread

**Output:**
- Execution continues in the selected thread at the saved instruction pointer

**ABI:**
- Implemented in assembly for x86_64
- Must follow System V calling convention
- Interrupts must be disabled during the actual context switch to prevent races

## Thread Creation

- Allocate thread structure and kernel stack
- Initialize saved context to make thread start at entry function with argument
- Set up initial stack frame
- Place thread in Runnable state
- Add to scheduler's runnable queue

## Thread Termination

- When a thread's entry function returns, it must go through a cleanup path
- Transition thread to Terminated state
- Do not re-schedule terminated threads
- Stack and resources remain allocated until explicitly freed (deferred to later)

## Scheduler Interface

Minimal interface for Milestone 7:
- `thread_create(entry, arg, name) -> ThreadId`
- `thread_yield()` - cooperative yield to next runnable thread
- `thread_exit()` - terminate current thread
- `schedule()` or internal selection of next runnable thread

Round-robin selection among Runnable threads is sufficient.

## Interrupt Interaction

**Policy for Milestone 7: Cooperative switching only**

- Context switches occur only when explicitly requested via `thread_yield()` or similar
- Timer interrupts must NOT trigger preemption in this milestone
- Interrupts must be disabled during the context switch critical section
- The timer remains functional for diagnostics but is not used for scheduling decisions
- Preemption will be integrated only after cooperative switching is proven stable

## Memory Ownership

- Thread structures allocated from kernel heap
- Kernel stacks allocated from kernel heap (via VM/PMM)
- All memory remains kernel-accessible
- No user memory involved

## Testing Strategy

### Host Unit Tests
- Thread state transitions (Created → Runnable → Running → Terminated)
- Stack allocation and alignment validation
- Context structure layout and size
- Thread creation logic

### QEMU Integration Test
Create two kernel threads that:
1. Print a marker (e.g., "thread A: start")
2. Yield to the other thread
3. Print another marker (e.g., "thread A: resume")
4. Repeat for a fixed number of switches
5. Terminate cleanly
6. Report success to QEMU

**Verification:**
- Each thread resumes at the correct instruction
- Registers and stack state survive switching
- Thread stacks do not overlap
- A terminated thread is not selected again
- Kernel does not rely on timer preemption

## Diagnostics

Add serial output for:
- Thread creation/destruction
- Context switches (optional, may be verbose)
- Current thread ID
- Thread state changes

## Out of Scope

- User mode
- Processes
- Address-space switching
- Preemptive scheduling
- SMP (Symmetric Multiprocessing)
- CPU affinity
- Blocking synchronization (mutexes, semaphores, condition variables)
- Signals
- Floating-point/SIMD context
- ARM64 implementation
- User stacks
- Dynamic stack growth
- Stack guard pages (documented for future)

## Exit Criteria

- [ ] Kernel-thread ownership is documented
- [ ] Thread states are defined (Runnable, Running, Terminated minimum)
- [ ] Stack layout is documented
- [ ] Saved CPU context is documented (caller/callee-saved split)
- [ ] Context-switch contract is documented with clear ABI
- [ ] A kernel thread can be created
- [ ] Two kernel threads can switch cooperatively
- [ ] Thread termination is handled safely
- [ ] Invalid thread states are rejected
- [ ] Stack alignment is validated (16-byte aligned)
- [ ] Context-switch unit tests pass
- [ ] QEMU thread-switch test passes
- [ ] Existing PMM, VM, heap, and interrupt tests pass
- [ ] Interrupt interaction policy is documented (cooperative first)
- [ ] Limitations are documented

**Central Rule:** Prove cooperative context switching before allowing timer interrupts to preempt arbitrary kernel code.
