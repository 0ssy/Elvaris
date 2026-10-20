//! Page fault handling and diagnostics.

#![allow(dead_code)]

use crate::serial;

/// Page fault error code bits.
pub mod error_code {
    pub const PRESENT: u32 = 1 << 0;
    pub const WRITE: u32 = 1 << 1;
    pub const USER: u32 = 1 << 2;
    pub const RESERVED_WRITE: u32 = 1 << 3;
    pub const INSTRUCTION_FETCH: u32 = 1 << 4;
}

/// Print page fault diagnostics.
///
/// The kernel has no exception handling yet, so a page fault is reported and
/// the CPU is halted.
pub fn handle_page_fault(fault_address: u64, error_code: u32) -> ! {
    unsafe {
        serial::write_str("Elvaris kernel: PAGE FAULT\r\n");
        serial::write_str("Elvaris kernel: fault address: ");
        serial::write_fmt(core::format_args!("0x{:x}", fault_address));
        serial::write_newline();
        serial::write_str("Elvaris kernel: error code: ");
        serial::write_fmt(core::format_args!("0x{:x}", error_code));
        serial::write_newline();
    }
    // Halt safely on page fault
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}
