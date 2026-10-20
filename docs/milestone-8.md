# Milestone 8: Preemptive Scheduler

## Scheduling Policy
Single CPU, fixed time slice, round-robin among runnable threads. Preempt on every timer tick (~100 Hz, local APIC timer) in addition to cooperative yielding.

## Timer Integration
Local APIC timer drives preemption. Tick counter incremented in timer handler. No heap allocation in interrupt path.

## Interrupt Context
Full GPR + interrupt frame (RIP, CS, RFLAGS, RSP, SS) preserved per Milestone 6's 176-byte layout in `cpu/vectors.rs`.

## Locking and Interrupt Safety
Scheduler protected by spinlock with interrupts disabled during critical sections. No locks held by interrupted code acquired in timer handler without care.

## Thread Lifecycle
Created -> Runnable -> Running -> (Runnable on preemption/yield) -> Terminated. Terminated stacks switched away before reclamation.

## Limitations
Single CPU only; no priorities, blocking, or sleeping states.
