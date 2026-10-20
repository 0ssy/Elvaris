//! Validation of the addresses a user program hands to the kernel.
//!
//! Every address a user program passes to the kernel is untrusted input. A
//! pointer is only usable when
//!
//! - it is a canonical x86-64 address,
//! - the whole range it describes lies inside the user address space,
//! - and every page of the range is mapped, accessible from user mode, and
//!   writable when the call has to write to it.
//!
//! The range checks are pure and are unit tested on the host. The page checks
//! walk the page tables of the active address space; they run in the kernel,
//! where the physical-memory direct map makes the tables reachable.

use crate::user::address::{USER_ADDRESS_SPACE_END, USER_ADDRESS_SPACE_START};
use crate::vmm::page_table::{entry_flags, is_canonical, page_start, PAGE_SIZE};
use crate::vmm::translate;

/// Why a user pointer was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerError {
    /// The address is not a canonical x86-64 address.
    NotCanonical,
    /// The address is outside the user address space.
    OutsideUserSpace,
    /// The range runs past the end of the address space.
    Overflow,
    /// A page of the range is not mapped.
    Unmapped,
    /// A page of the range is not accessible from user mode.
    NotUserAccessible,
    /// A page of the range is not writable, but the call has to write to it.
    NotWritable,
}

impl PointerError {
    /// A short description of the error, used in the serial diagnostics.
    pub const fn description(&self) -> &'static str {
        match self {
            PointerError::NotCanonical => "address is not canonical",
            PointerError::OutsideUserSpace => "address is outside the user address space",
            PointerError::Overflow => "the range runs past the end of the address space",
            PointerError::Unmapped => "a page of the range is not mapped",
            PointerError::NotUserAccessible => {
                "a page of the range is not accessible from user mode"
            }
            PointerError::NotWritable => "a page of the range is not writable",
        }
    }
}

/// Check that the range lies completely inside the user address space.
///
/// A range of length zero is always accepted: it describes no memory at all,
/// so there is nothing the kernel could dereference.
pub fn validate_range(address: u64, length: u64) -> Result<(), PointerError> {
    if length == 0 {
        return Ok(());
    }
    if !is_canonical(address) {
        return Err(PointerError::NotCanonical);
    }
    let end = address.checked_add(length).ok_or(PointerError::Overflow)?;
    if address < USER_ADDRESS_SPACE_START || end > USER_ADDRESS_SPACE_END {
        return Err(PointerError::OutsideUserSpace);
    }
    Ok(())
}

/// The number of pages the range touches.
///
/// The range has to be valid; use [`validate_range`] first.
pub fn page_count(address: u64, length: u64) -> usize {
    if length == 0 {
        return 0;
    }
    let first = page_start(address);
    let last = page_start(address + length - 1);
    ((last - first) / PAGE_SIZE + 1) as usize
}

/// Check the permissions of a single page table entry.
///
/// The flags are the permission bits of the entry: the low twelve bits and
/// the no-execute bit. The kernel only cares about the present bit, the
/// user bit and the writable bit here.
pub fn check_page_flags(flags: u64, need_write: bool) -> Result<(), PointerError> {
    if flags & entry_flags::PRESENT == 0 {
        return Err(PointerError::Unmapped);
    }
    if flags & entry_flags::USER_ACCESSIBLE == 0 {
        return Err(PointerError::NotUserAccessible);
    }
    if need_write && flags & entry_flags::WRITABLE == 0 {
        return Err(PointerError::NotWritable);
    }
    Ok(())
}

/// Check that every page of the range is mapped and usable by the kernel on
/// behalf of the user program.
///
/// `need_write` asks for a range the kernel has to write to; a read-only
/// range is enough for a call that only reads.
pub fn validate_access(address: u64, length: u64, need_write: bool) -> Result<(), PointerError> {
    validate_range(address, length)?;
    if length == 0 {
        return Ok(());
    }

    let first = page_start(address);
    let last = page_start(address + length - 1);
    let mut page = first;
    loop {
        let Some((_, flags)) = translate::translate_current(page) else {
            return Err(PointerError::Unmapped);
        };
        check_page_flags(flags, need_write)?;
        if page == last {
            break;
        }
        page += PAGE_SIZE;
    }
    Ok(())
}
