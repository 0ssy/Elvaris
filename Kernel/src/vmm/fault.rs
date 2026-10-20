//! Page fault handling and diagnostics.
//!
//! A page fault is CPU exception 14. Once the interrupt descriptor table is
//! installed, the fault reaches the unified exception handler of the CPU
//! module, which prints the diagnostic and terminates the machine.
//!
//! This module keeps the bits of the page-fault error code and offers a
//! direct entry point for the case where the kernel detects a fault itself.

use crate::cpu::exception;

/// Bits of the page-fault error code.
pub mod error_code {
    #[allow(unused_imports)]
    pub use crate::cpu::vectors::page_fault::*;
}

/// Handle a page fault that the kernel detected itself.
///
/// `fault_address` is the address that could not be accessed and
/// `error_code` is the page-fault error code.
///
/// The diagnostic is the same one the unified exception path prints: the
/// exception name, the error code, the instruction pointer, the stack pointer
/// and the faulting address.
pub fn handle_page_fault(fault_address: u64, error_code: u32) -> ! {
    exception::report_faulting_page_fault(fault_address, error_code as u64)
}
