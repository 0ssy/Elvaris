//! The kernel timer and the interrupt counters.
//!
//! The timer interrupt is the kernel's first source of asynchronous events.
//! The counters are updated with atomic operations, so an interrupt handler
//! can read them without taking a lock.

use core::sync::atomic::{AtomicU64, Ordering};

/// Number of timer interrupts received.
static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);

/// Number of hardware interrupts received.
static HARDWARE_INTERRUPTS: AtomicU64 = AtomicU64::new(0);

/// Number of interrupts received in total.
static TOTAL_INTERRUPTS: AtomicU64 = AtomicU64::new(0);

/// Number of spurious interrupts received from the local APIC.
static SPURIOUS_INTERRUPTS: AtomicU64 = AtomicU64::new(0);

/// Handle a timer interrupt.
pub fn handle_timer_interrupt() {
    TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
    TOTAL_INTERRUPTS.fetch_add(1, Ordering::Relaxed);
    // Acknowledge the interrupt at the local APIC. The processor keeps the
    // interrupt in its in-service register until the kernel acknowledges it,
    // and it stops delivering interrupts of the same priority until then.
    crate::cpu::apic::end_of_interrupt();
}

/// Handle a spurious interrupt.
///
/// A spurious interrupt is an interrupt that was already delivered while the
/// processor was busy. The kernel counts it and acknowledges it, so that it
/// does not disturb the interrupt state.
pub fn handle_spurious_interrupt() {
    SPURIOUS_INTERRUPTS.fetch_add(1, Ordering::Relaxed);
    TOTAL_INTERRUPTS.fetch_add(1, Ordering::Relaxed);
    crate::cpu::apic::end_of_interrupt();
}

/// The number of spurious interrupts received.
pub fn spurious_interrupts() -> u64 {
    SPURIOUS_INTERRUPTS.load(Ordering::Relaxed)
}

/// Count a hardware interrupt.
pub fn count_hardware_interrupt() {
    HARDWARE_INTERRUPTS.fetch_add(1, Ordering::Relaxed);
    TOTAL_INTERRUPTS.fetch_add(1, Ordering::Relaxed);
}

/// The number of timer interrupts received.
pub fn timer_ticks() -> u64 {
    TIMER_TICKS.load(Ordering::Relaxed)
}

/// The number of hardware interrupts received.
pub fn hardware_interrupts() -> u64 {
    HARDWARE_INTERRUPTS.load(Ordering::Relaxed)
}

/// The number of interrupts received in total.
pub fn total_interrupts() -> u64 {
    TOTAL_INTERRUPTS.load(Ordering::Relaxed)
}

/// Reset every counter.
pub fn reset_counters() {
    TIMER_TICKS.store(0, Ordering::Relaxed);
    HARDWARE_INTERRUPTS.store(0, Ordering::Relaxed);
    TOTAL_INTERRUPTS.store(0, Ordering::Relaxed);
    SPURIOUS_INTERRUPTS.store(0, Ordering::Relaxed);
}
