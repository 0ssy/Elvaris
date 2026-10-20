//! The local APIC.
//!
//! The local APIC is the processor's own interrupt controller. It is always
//! present on a modern processor, and it is the source of the timer
//! interrupt the kernel uses.
//!
//! The registers are memory mapped at a fixed physical address. The kernel
//! reaches them through a mapping it installs itself: the bootloader maps
//! the usable physical memory, and the local APIC registers live outside of
//! it. The kernel maps the page at the same address in the virtual address
//! space, which is the conventional choice.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::cpu::cpuid;

/// The physical address of the local APIC registers.
pub const LAPIC_PHYSICAL_BASE: u64 = 0xFEE0_0000;

/// The virtual address the kernel maps the local APIC registers at.
pub const LAPIC_VIRTUAL_BASE: u64 = 0xFEE0_0000;

/// Offset of the identification register.
pub const OFFSET_ID: u64 = 0x020;

/// Offset of the version register.
pub const OFFSET_VERSION: u64 = 0x030;

/// Offset of the spurious interrupt vector register.
pub const OFFSET_SVR: u64 = 0x0F0;

/// Offset of the task priority register.
pub const OFFSET_TPR: u64 = 0x080;

/// Offset of the end-of-interrupt register.
pub const OFFSET_EOI: u64 = 0x0B0;

/// Offset of the local vector table entry of the timer.
pub const OFFSET_LVT_TIMER: u64 = 0x320;

/// Offset of the initial count register of the timer.
pub const OFFSET_TIMER_ICR: u64 = 0x380;

/// Offset of the current count register of the timer.
pub const OFFSET_TIMER_CCR: u64 = 0x390;

/// Offset of the divide configuration register of the timer.
pub const OFFSET_TIMER_DIVIDE: u64 = 0x3E0;

/// Bit 8 of the spurious interrupt vector register: the local APIC is enabled.
pub const SVR_ENABLE: u32 = 1 << 8;

/// The vector the local APIC reports a spurious interrupt with.
///
/// A spurious interrupt is an interrupt that was already delivered while the
/// processor was busy. The kernel installs a handler for this vector, so a
/// spurious interrupt is counted instead of taking the machine down.
pub const SPURIOUS_VECTOR: u8 = 0xFF;

/// Bit 16 of the local vector table entry of the timer: the interrupt is
/// masked.
pub const LVT_TIMER_MASK: u32 = 1 << 16;

/// The bits of the local vector table entry of the timer the kernel sets.
///
/// The vector lives in bits 7 to 0, the mask bit in bit 16 and the timer mode
/// in bits 18 and 17. The kernel clears all of them before it writes the
/// vector and the mode.
pub const LVT_TIMER_RESET_MASK: u32 = 0x0007_FFFF;

/// The frequency of the local APIC timer bus, in hertz.
///
/// The timer bus of the local APIC runs at a fixed frequency that the
/// processor does not report. In the machine the kernel is tested on, the
/// frequency is 100 MHz. The timer is programmed with a divide factor of 16,
/// which leaves 6.25 MHz and a maximum period of about 11 minutes.
pub const LAPIC_BUS_FREQUENCY: u32 = 100_000_000;

/// The divide factor used by the timer.
pub const TIMER_DIVIDE: u32 = 16;

/// The timer mode that makes the timer periodic.
///
/// The mode field of the local vector table entry of the timer lives in bits
/// 18 and 17; `0b01` selects the periodic mode.
pub const TIMER_MODE_PERIODIC: u32 = 0b01 << 17;

/// The divisor value that selects a divide factor of 16.
pub const DIVIDE_BY_16: u32 = 0b0011;

/// The virtual address of the local APIC registers, zero when they are not
/// mapped yet.
static BASE: AtomicU64 = AtomicU64::new(0);

/// Record the address the local APIC registers are mapped at.
///
/// The kernel has to map the page before it can read or write the
/// registers.
pub fn set_base(address: u64) {
    BASE.store(address, Ordering::Relaxed);
}

/// The address the local APIC registers are mapped at.
///
/// Zero means that the page is not mapped and that the registers cannot be
/// accessed.
pub fn base_address() -> u64 {
    BASE.load(Ordering::Relaxed)
}

/// Whether the local APIC registers are accessible.
pub fn is_mapped() -> bool {
    base_address() != 0
}

/// Read a register of the local APIC.
fn read_register(offset: u64) -> u32 {
    let base = base_address();
    if base == 0 {
        return 0;
    }
    unsafe { core::ptr::read_volatile((base + offset) as *const u32) }
}

/// Write a register of the local APIC.
fn write_register(offset: u64, value: u32) {
    let base = base_address();
    if base == 0 {
        return;
    }
    unsafe { core::ptr::write_volatile((base + offset) as *mut u32, value) }
}

/// Whether the local APIC is present.
pub fn is_present() -> bool {
    cpuid::has_local_apic()
}

/// The identification of the local APIC.
pub fn id() -> u32 {
    read_register(OFFSET_ID) >> 24
}

/// The version of the local APIC.
pub fn version() -> u32 {
    read_register(OFFSET_VERSION)
}

/// Enable the local APIC.
///
/// The spurious interrupt vector is set to 0xFF, which tells the processor
/// which vector to use for a spurious interrupt; the kernel counts those
/// instead of treating them as an error.
pub fn enable() {
    // The task priority register decides which interrupts the processor
    // accepts. It resets to zero, but the bootloader may have left a value
    // behind, so the kernel sets it explicitly.
    write_register(OFFSET_TPR, 0);

    let svr = read_register(OFFSET_SVR);
    write_register(
        OFFSET_SVR,
        ((svr | SVR_ENABLE) & !0xFF) | SPURIOUS_VECTOR as u32,
    );
}

/// Whether the local APIC reports itself as enabled.
pub fn is_enabled() -> bool {
    read_register(OFFSET_SVR) & SVR_ENABLE != 0
}

/// Acknowledge an interrupt the local APIC delivered.
///
/// The processor keeps the interrupt in its in-service register until the
/// kernel acknowledges it. Without the acknowledgement the local APIC stops
/// delivering interrupts of the same priority.
pub fn end_of_interrupt() {
    write_register(OFFSET_EOI, 0);
}

/// The value of the local vector table entry of the timer.
///
/// The vector lives in bits 7 to 0, the mode in bits 18 and 17. The mask bit
/// in bit 16 stays clear, so that the timer raises its interrupt.
pub const fn lvt_timer_value(vector: u8, mode: u32) -> u32 {
    (mode & 0x0006_0000) | (vector as u32 & 0xFF)
}

/// Program the timer to interrupt `vector` with the given period.
///
/// The initial count is derived from the bus frequency and the divide factor.
/// A count of zero means "the timer is stopped".
pub fn program_timer(vector: u8, frequency: u32) -> Option<u32> {
    if frequency == 0 {
        return None;
    }
    let ticks = LAPIC_BUS_FREQUENCY / TIMER_DIVIDE;
    let count = ticks / frequency;
    if count == 0 || count > u32::MAX as u32 {
        return None;
    }

    // Select the divide factor.
    let divide = read_register(OFFSET_TIMER_DIVIDE);
    write_register(OFFSET_TIMER_DIVIDE, (divide & !0b1111) | DIVIDE_BY_16);

    // Install the vector and select the periodic mode. The mask bit is
    // cleared, otherwise the timer never raises an interrupt.
    let lvtt = read_register(OFFSET_LVT_TIMER);
    write_register(
        OFFSET_LVT_TIMER,
        (lvtt & !LVT_TIMER_RESET_MASK) | lvt_timer_value(vector, TIMER_MODE_PERIODIC),
    );

    // Start the timer.
    write_register(OFFSET_TIMER_ICR, count);
    Some(count)
}

/// The current count of the timer.
pub fn timer_count() -> u32 {
    read_register(OFFSET_TIMER_CCR)
}

/// Stop the timer.
pub fn stop_timer() {
    write_register(OFFSET_TIMER_ICR, 0);
}
