//! CPU and interrupt infrastructure.
//!
//! This module brings up everything the kernel needs to receive events from
//! the processor:
//!
//! - a global descriptor table with a code segment, a data segment, the
//!   user-mode segments and a task state segment,
//! - a task state segment with a ring 0 stack and a dedicated double-fault
//!   stack,
//! - an interrupt descriptor table with a handler for every CPU exception,
//! - the legacy interrupt controllers, remapped so that no hardware
//!   interrupt can collide with an exception vector,
//! - the local APIC, enabled,
//! - a timer that produces a periodic interrupt.
//!
//! The order matters: the global descriptor table and the task state segment
//! come first, because the interrupt stubs run in the kernel code segment;
//! then the interrupt descriptor table, because an exception before it is
//! installed would fault; then the interrupt controllers and the timer; and
//! only then are interrupts enabled.

#![allow(dead_code)]

pub mod apic;
pub mod cpuid;
pub mod descriptor;
pub mod exception;
pub mod gdt;
pub mod idt;
pub mod interrupts;
pub mod pic;
pub mod pit;
pub mod state;
pub mod timer;
pub mod tss;
pub mod vectors;

#[cfg(test)]
mod tests;

use crate::serial;

/// The frequency of the kernel timer, in hertz.
///
/// A hundred interrupts per second are slow enough to keep the diagnostic
/// output readable and fast enough to be useful.
pub const TIMER_FREQUENCY: u32 = 100;

/// The vector the local APIC timer uses.
pub const TIMER_VECTOR: u8 = vectors::TIMER_VECTOR;

/// Page-table flags used for the memory mapped device registers.
///
/// The registers are present and writable, and they are never executable.
pub const MMIO_PAGE_FLAGS: u64 =
    crate::vmm::page_table::entry_flags::PRESENT
        | crate::vmm::page_table::entry_flags::WRITABLE
        | crate::vmm::page_table::entry_flags::NO_EXECUTE;

/// What the initialization installed.
#[derive(Debug, Clone, Copy)]
pub struct CpuInit {
    /// Whether the local APIC is present and was enabled.
    pub apic_enabled: bool,
    /// The initial count of the local APIC timer.
    pub apic_timer_count: u32,
    /// The divisor of the legacy timer, if it was programmed.
    pub pit_divisor: Option<u16>,
    /// The vector the legacy timer interrupt uses.
    pub pit_vector: u8,
}

/// Install the CPU infrastructure.
///
/// `kernel_stack_top` is the top of the stack the processor must switch to
/// when an event arrives from user mode. The interrupts are still disabled
/// when this function returns; [`enable_interrupts`] turns them on.
pub fn init(kernel_stack_top: u64) -> CpuInit {
    // The task state segment has to exist before the global descriptor table
    // is loaded, because the table holds its descriptor.
    tss::prepare(kernel_stack_top);
    unsafe {
        serial::write_str("Elvaris kernel: cpu: tss prepared\r\n");
    }

    // SAFETY: the task state segment was prepared above and the interrupts
    // are disabled, so no event can arrive while the segment registers are
    // reloaded.
    unsafe { gdt::install(tss::base_address(), tss::limit()) };
    unsafe {
        serial::write_str("Elvaris kernel: cpu: gdt installed\r\n");
    }

    // Install the handlers. The double fault uses the dedicated stack.
    let mut table = idt::Idt::new();
    for (vector, stub) in exception::EXCEPTION_STUBS.iter().enumerate() {
        let vector = vector as u8;
        let ist = exception::ist_for_vector(vector);
        table.set_handler(vector, *stub as u64, exception::handler_selector(), ist);
    }
    for (irq, stub) in exception::HARDWARE_STUBS.iter().enumerate() {
        let vector = vectors::HARDWARE_VECTOR_BASE + irq as u8;
        table.set_handler(vector, *stub as u64, exception::handler_selector(), 0);
    }
    table.set_handler(
        TIMER_VECTOR,
        exception::TIMER_STUB as u64,
        exception::handler_selector(),
        0,
    );
    table.set_handler(
        apic::SPURIOUS_VECTOR,
        exception::SPURIOUS_STUB as u64,
        exception::handler_selector(),
        0,
    );

    // SAFETY: the table lives in a static, so it stays valid for as long as
    // the processor uses it.
    unsafe { idt::install(table) };

    // The legacy interrupt controllers are remapped so that the hardware
    // interrupts start at 0x20, behind the exception vectors.
    let pic_state = pic::init();

    // The local APIC is enabled; the timer becomes the tick source.
    let apic_present = apic::is_present();
    let mut apic_timer_count = 0;
    if apic_present {
        apic::enable();
        apic_timer_count = apic::program_timer(TIMER_VECTOR, TIMER_FREQUENCY).unwrap_or(0);
    }

    // The legacy timer is the second source. It is programmed, and its
    // interrupt line is unmasked so that the kernel can see which of the two
    // sources delivers the tick in this machine.
    let pit_divisor = unsafe { pit::set_frequency(TIMER_FREQUENCY) };
    if pit_divisor.is_some() {
        pic::unmask_irq(0);
    }

    unsafe {
        serial::write_str("Elvaris kernel: cpu: gdt installed base=0x");
        serial::write_fmt(core::format_args!("{:x}", gdt::base_address()));
        serial::write_str(" limit=");
        serial::write_fmt(core::format_args!("{}", gdt::limit()));
        serial::write_newline();
        serial::write_str("Elvaris kernel: cpu: tss installed base=0x");
        serial::write_fmt(core::format_args!("{:x}", tss::base_address()));
        serial::write_str(" limit=");
        serial::write_fmt(core::format_args!("{}", tss::limit()));
        serial::write_str(" ist1=0x");
        serial::write_fmt(core::format_args!("{:x}", tss::double_fault_stack_top()));
        serial::write_newline();
        serial::write_str("Elvaris kernel: cpu: idt installed base=0x");
        serial::write_fmt(core::format_args!("{:x}", idt::base_address()));
        serial::write_str(" limit=");
        serial::write_fmt(core::format_args!("{}", idt::limit()));
        serial::write_newline();
        serial::write_str("Elvaris kernel: cpu: pic remapped master=0x");
        serial::write_fmt(core::format_args!("{:x}", pic_state.master_base));
        serial::write_str(" slave=0x");
        serial::write_fmt(core::format_args!("{:x}", pic_state.slave_base));
        serial::write_newline();
    }

    CpuInit {
        apic_enabled: apic_present,
        apic_timer_count,
        pit_divisor,
        pit_vector: pic_state.master_base,
    }
}

/// Enable the interrupts.
///
/// Every handler is installed at this point, so the processor can deliver
/// events safely.
pub fn enable_interrupts() {
    unsafe { interrupts::enable() };
}

/// Disable the interrupts and return the previous state.
pub fn disable_interrupts() -> bool {
    unsafe { interrupts::disable() }
}

/// Restore a previously saved interrupt state.
pub fn restore_interrupts(enabled: bool) {
    unsafe { interrupts::restore(enabled) };
}

/// Wait until the timer produced at least `target` ticks.
///
/// The wait is bounded: the function returns the number of ticks it saw. A
/// return value below the target means that the timer did not work, and the
/// caller decides whether that is fatal.
pub fn wait_for_timer_ticks(target: u64, max_iterations: u64) -> u64 {
    for _ in 0..max_iterations {
        if timer::timer_ticks() >= target {
            break;
        }
        core::hint::spin_loop();
    }
    timer::timer_ticks()
}
