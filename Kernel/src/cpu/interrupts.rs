//! Interrupt enable and disable helpers.
//!
//! The kernel starts with interrupts disabled: the bootloader hands control
//! over with the interrupt-enable flag (IF) cleared. Interrupts stay disabled
//! until the GDT, the TSS, the IDT and the timer are installed, because an
//! interrupt that arrives before the IDT is in place would fault.
//!
//! Once interrupts are enabled, code that must not be interrupted — anything
//! that holds a lock, for example — uses [`disable`] and [`restore`].

/// Whether the interrupt-enable flag is currently set.
pub fn are_enabled() -> bool {
    let flags: u64;
    unsafe {
        // The push and the pop are balanced, so the stack pointer is the same
        // before and after. The asm does touch memory (the stack), so it must
        // not claim otherwise.
        core::arch::asm!(
            "pushfq",
            "pop {0}",
            out(reg) flags,
            options(nostack)
        );
    }
    flags & (1 << 9) != 0
}

/// Disable interrupts and return the previous interrupt-enable state.
///
/// The caller must restore the previous state with [`restore`]. Returning the
/// state instead of unconditionally re-enabling interrupts keeps the
/// operation safe in code that already runs with interrupts disabled.
///
/// # Safety
///
/// Interrupts stay disabled until [`restore`] is called. Every exit path of
/// the caller must restore them.
pub unsafe fn disable() -> bool {
    let enabled = are_enabled();
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
    }
    enabled
}

/// Enable interrupts.
///
/// # Safety
///
/// The caller must have installed an IDT with a handler for every event that
/// can occur, because interrupts become deliverable immediately.
pub unsafe fn enable() {
    unsafe {
        core::arch::asm!("sti", options(nomem, nostack));
    }
}

/// Restore a previously saved interrupt-enable state.
///
/// # Safety
///
/// `enabled` must be the value returned by [`disable`].
pub unsafe fn restore(enabled: bool) {
    if enabled {
        unsafe { enable() };
    }
}

/// Execute `sti` immediately followed by `hlt`.
///
/// The processor guarantees that at least one interrupt is processed between
/// the `sti` and the `hlt`, so the halt cannot miss a pending interrupt. This
/// is the canonical way to wait for an interrupt without spinning.
///
/// # Safety
///
/// Interrupts must be safe to receive at this point.
pub unsafe fn sti_hlt() {
    unsafe {
        core::arch::asm!("sti; hlt", options(nomem, nostack));
    }
}
