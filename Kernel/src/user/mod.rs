//! User mode.
//!
//! This milestone starts exactly one user program: the kernel creates an
//! address space for it, loads a fixed image into it, enters ring 3, and
//! takes the program down again when it ends.
//!
//! The kernel keeps ownership of the program for its whole lifetime. The
//! program never owns memory the kernel cannot take back, and every pointer
//! it hands to the kernel is validated before the kernel uses it.

pub mod address;
pub mod pointer;
pub mod program;
pub mod session;
pub mod syscall;

#[cfg(test)]
mod tests;

use crate::serial;
use crate::user::address::{UserRegion, REGIONS, USER_CODE_FLAGS, USER_DATA_FLAGS, USER_STACK_TOP};
use crate::user::session::Outcome;
use crate::vmm::mapper::{MapError, Mapper};
use crate::vmm::page_table::PAGE_SIZE;

/// The size of the stack the processor switches to when an event arrives from
/// user mode.
///
/// The stack lives in the kernel heap and belongs to the kernel: the user
/// program never touches it.
pub const USER_INTERRUPT_STACK_SIZE: usize = 64 * 1024;

/// Page-table flags the user code page is mapped with while the program is
/// loaded. The page has to be writable for the kernel to put the program
/// into it.
pub const USER_CODE_WRITE_FLAGS: u64 =
    crate::vmm::page_table::entry_flags::PRESENT
        | crate::vmm::page_table::entry_flags::WRITABLE
        | crate::vmm::page_table::entry_flags::USER_ACCESSIBLE;

/// Errors of the user-mode setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserError {
    /// The virtual-memory subsystem refused a mapping.
    Mapping(MapError),
    /// The kernel heap could not provide the stack for user-mode interrupts.
    OutOfMemory,
    /// The user program does not fit into the user code page.
    ProgramTooLarge,
    /// The self test of the user-mode subsystem did not behave as documented.
    SelfTestFailed,
}

impl From<MapError> for UserError {
    fn from(err: MapError) -> Self {
        UserError::Mapping(err)
    }
}

/// Allocate the stack the processor uses for the events that arrive from user
/// mode.
///
/// The block stays allocated for as long as the kernel runs: the task state
/// segment points at it for the whole lifetime of the user program.
pub fn allocate_interrupt_stack() -> Result<u64, UserError> {
    let block = crate::heap::alloc(USER_INTERRUPT_STACK_SIZE, 16).map_err(|_| UserError::OutOfMemory)?;
    Ok(block.as_ptr() as u64 + USER_INTERRUPT_STACK_SIZE as u64)
}

/// Map the user address space.
///
/// The regions are mapped with the permissions of their role: the code page
/// is executable and readable, the data page and the stack are writable.
pub fn create_address_space(mapper: &mut Mapper) -> Result<(), UserError> {
    for region in REGIONS {
        let flags = match region {
            UserRegion::Code { .. } => USER_CODE_WRITE_FLAGS,
            _ => USER_DATA_FLAGS,
        };
        mapper.map_range(region.base(), region.pages(), flags)?;
    }
    Ok(())
}

/// Copy the user program and its message into the user address space.
///
/// The code page is read-only once the program is in it: the user program
/// cannot change the code the processor runs.
pub fn load_program(mapper: &mut Mapper) -> Result<(), UserError> {
    let image = program::image();
    if image.len() > address::USER_CODE_PAGES * PAGE_SIZE as usize {
        return Err(UserError::ProgramTooLarge);
    }

    // SAFETY: the user code and data pages are mapped and writable, and the
    // image fits into the code page.
    unsafe {
        core::ptr::copy_nonoverlapping(
            image.as_ptr(),
            address::USER_CODE_BASE as *mut u8,
            image.len(),
        );
        core::ptr::copy_nonoverlapping(
            program::MESSAGE.as_ptr(),
            address::USER_DATA_BASE as *mut u8,
            program::MESSAGE.len(),
        );
    }

    // The code page becomes read-only: the program may read it and run it,
    // but it cannot write to it.
    mapper.update_flags(address::USER_CODE_BASE, USER_CODE_FLAGS)?;

    Ok(())
}

/// Prepare the user address space and run the user program.
pub fn start(mapper: &mut Mapper) -> Result<Outcome, UserError> {
    create_address_space(mapper)?;
    load_program(mapper)?;

    unsafe {
        serial::write_str("Elvaris kernel: user: starting the user program\r\n");
        serial::write_str("Elvaris kernel: user: entry=0x");
        serial::write_fmt(core::format_args!("{:x}", program::entry_address()));
        serial::write_str(" stack_top=0x");
        serial::write_fmt(core::format_args!("{:x}", USER_STACK_TOP));
        serial::write_newline();
    }

    report_mappings();

    let outcome = session::run(
        program::entry_address(),
        USER_STACK_TOP,
        address::USER_DATA_BASE,
        program::MESSAGE.len() as u64,
    );
    unsafe {
        serial::write_str("Elvaris kernel: user: the user program ended\r\n");
    }

    Ok(outcome)
}

/// Report the layout of the user address space.
pub fn report() {
    unsafe {
        serial::write_str("Elvaris kernel: user: address space: start=0x");
        serial::write_fmt(core::format_args!("{:x}", address::USER_ADDRESS_SPACE_START));
        serial::write_str(" end=0x");
        serial::write_fmt(core::format_args!("{:x}", address::USER_ADDRESS_SPACE_END));
        serial::write_newline();
        for region in REGIONS {
            serial::write_str("Elvaris kernel: user: region: base=0x");
            serial::write_fmt(core::format_args!("{:x}", region.base()));
            serial::write_str(" pages=");
            serial::write_fmt(core::format_args!("{}", region.pages()));
            serial::write_newline();
        }
    }
}

/// Report the page-table entries of the first page of every region.
///
/// The report shows the raw entry of every level of the page-table walk, so
/// a permission that the processor refuses can be traced to the level that
/// carries it.
pub fn report_mappings() {
    for region in REGIONS {
        let base = region.base();
        let entries = crate::vmm::translate::walk_current(base);
        unsafe {
            serial::write_str("Elvaris kernel: user: mapping: base=0x");
            serial::write_fmt(core::format_args!("{:x}", base));
            for entry in entries {
                serial::write_str(" entry=0x");
                serial::write_fmt(core::format_args!("{:x}", entry));
            }
            serial::write_newline();
        }
    }
}

/// Run the self test of the user-mode subsystem.
///
/// The test runs the two probes of the user image. The first one reads from
/// an address that is not mapped, and the kernel has to take it down with a
/// page fault without the machine going down with it. The second one raises a
/// system call the kernel does not know, and the kernel has to refuse it and
/// take the program down as well.
///
/// The test fails when a probe ends in a way the kernel did not expect: a
/// program that keeps running after it caused a fault, or one that the
/// kernel accepted a refused system call for.
pub fn self_test() -> Result<(), UserError> {
    // The fault probe: the program reads from the first address above the
    // user address space. The address is not mapped, so the processor raises
    // a page fault.
    let outcome = session::run(
        program::fault_probe_address(),
        USER_STACK_TOP,
        address::USER_DATA_BASE,
        program::MESSAGE.len() as u64,
    );
    match outcome {
        Outcome::Faulted(vector) if vector == crate::cpu::vectors::vector::PAGE_FAULT => {
            unsafe {
                serial::write_str("Elvaris kernel: user: self_test: fault probe ok\r\n");
            }
        }
        _ => {
            unsafe {
                serial::write_str("Elvaris kernel: user: self_test: fault probe failed\r\n");
                report_outcome(outcome);
            }
            return Err(UserError::SelfTestFailed);
        }
    }

    // The unknown-system-call probe: the program raises a system call the
    // kernel does not know. The kernel refuses it.
    let outcome = session::run(
        program::unknown_syscall_probe_address(),
        USER_STACK_TOP,
        address::USER_DATA_BASE,
        program::MESSAGE.len() as u64,
    );
    match outcome {
        Outcome::Rejected(number) if number == program::unknown_syscall_number() => {
            unsafe {
                serial::write_str("Elvaris kernel: user: self_test: unknown syscall probe ok\r\n");
            }
        }
        _ => {
            unsafe {
                serial::write_str(
                    "Elvaris kernel: user: self_test: unknown syscall probe failed\r\n",
                );
                report_outcome(outcome);
            }
            return Err(UserError::SelfTestFailed);
        }
    }

    unsafe {
        serial::write_str("Elvaris kernel: user: self_test ok\r\n");
    }
    Ok(())
}

/// Report the way a user program ended.
pub fn report_outcome(outcome: Outcome) {
    unsafe {
        serial::write_str("Elvaris kernel: user: outcome=");
        match outcome {
            Outcome::Exited(code) => {
                serial::write_str("exited code=");
                serial::write_fmt(core::format_args!("{}", code));
                serial::write_newline();
            }
            Outcome::Faulted(vector) => {
                serial::write_str("faulted vector=");
                serial::write_fmt(core::format_args!("{}", vector));
                serial::write_newline();
            }
            Outcome::Rejected(detail) => {
                serial::write_str("rejected detail=");
                serial::write_fmt(core::format_args!("{}", detail));
                serial::write_newline();
            }
            Outcome::Running => {
                serial::write_str("running\r\n");
            }
        }
    }
}
