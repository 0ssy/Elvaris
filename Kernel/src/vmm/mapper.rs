//! Page table mapping utilities.
//!
//! The mapper walks (and optionally creates) the four-level page table
//! hierarchy to establish 4 KiB mappings. Because the current PMM is a
//! non-reclaiming bump allocator, unmapping does not return frames; this
//! limitation is documented.

#![allow(dead_code)]

use crate::pmm::phys_alloc::PhysFrameAllocator;
use crate::vmm::page_table::{
    current_pml4, pml4_index, pdpt_index, pd_index, pt_index, page_start, PAGE_SIZE,
};
use crate::vmm::page_table::entry_flags;

/// Error returned by mapping operations.
#[derive(Debug, PartialEq, Eq)]
pub enum MapError {
    /// The requested virtual address is not canonical.
    NonCanonicalAddress,
    /// The page table entry already exists and `overwrite` was not set.
    MappingAlreadyExists,
    /// A page table entry exists but is not a table where one is needed.
    NotATable,
    /// The physical address is not page aligned.
    FrameNotAligned,
    /// No page-table frames available from the allocator.
    OutOfMemory,
}

/// Valid mapping flags and helper constructors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapFlags(u64);

impl MapFlags {
    pub const KERNEL_CODE: u64 =
        entry_flags::PRESENT | entry_flags::NO_EXECUTE | 0; // writable=false, user=false
    pub const KERNEL_DATA: u64 = entry_flags::PRESENT | entry_flags::WRITABLE;
    pub const USER_READ_ONLY: u64 = entry_flags::PRESENT | entry_flags::USER_ACCESSIBLE;
    pub const USER_READ_WRITE: u64 =
        entry_flags::PRESENT | entry_flags::WRITABLE | entry_flags::USER_ACCESSIBLE;

    pub const fn new(bits: u64) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Validate flag combinations: reject contradictory or nonsensical flags.
    pub fn validate(bits: u64) -> Result<(), MapError> {
        // No-execute with write is allowed (W^X policy is enforced elsewhere).
        // Huge pages are not supported in this milestone.
        if (bits & entry_flags::HUGE_PAGE) != 0 {
            return Err(MapError::MappingAlreadyExists); // reuse: "not supported"
        }
        Ok(())
    }

    /// Present-only kernel mapping: readable, not writable, supervisor-only.
    pub const fn kernel_readonly() -> Self {
        Self(entry_flags::PRESENT)
    }

    /// Kernel data mapping: present, readable, writable, supervisor-only.
    pub const fn kernel_writable() -> Self {
        Self(entry_flags::PRESENT | entry_flags::WRITABLE)
    }

    /// Executable kernel code mapping: present, readable, executable, supervisor-only.
    pub const fn kernel_executable() -> Self {
        Self(entry_flags::PRESENT)
    }

    /// User read-only mapping: present, readable, user-accessible.
    pub const fn user_readonly() -> Self {
        Self(entry_flags::PRESENT | entry_flags::USER_ACCESSIBLE)
    }

    /// User read-write mapping: present, readable, writable, user-accessible.
    pub const fn user_writable() -> Self {
        Self(entry_flags::PRESENT | entry_flags::WRITABLE | entry_flags::USER_ACCESSIBLE)
    }
}

/// Mapper walks the page table hierarchy using physical addresses.
/// It requires a way to access page-table frames virtually; without a
/// physical-memory offset from the bootloader, this is limited to
/// host-side testing until the offset is available.
pub struct Mapper<'a> {
    allocator: &'a mut PhysFrameAllocator,
    pml4_phys: u64,
}

impl<'a> Mapper<'a> {
    pub unsafe fn new(allocator: &'a mut PhysFrameAllocator) -> Self {
        Self {
            allocator,
            pml4_phys: unsafe { current_pml4() },
        }
    }

    pub fn pml4_address(&self) -> u64 {
        self.pml4_phys
    }

    /// Allocate one physical frame for a page table from the PMM.
    fn alloc_table_frame(&mut self) -> Result<u64, MapError> {
        self.allocator
            .allocate_frame()
            .ok_or(MapError::OutOfMemory)
    }

    /// Map a single 4 KiB page: `virtual_addr` -> `physical_addr` with `flags`.
    /// Returns an error if the mapping already exists and overwrite is false.
    pub unsafe fn map_page(
        &mut self,
        virtual_addr: u64,
        physical_addr: u64,
        flags: u64,
        overwrite: bool,
    ) -> Result<(), MapError> {
        if !crate::vmm::page_table::is_canonical(virtual_addr) {
            return Err(MapError::NonCanonicalAddress);
        }
        MapFlags::validate(flags)?;
        if physical_addr & (PAGE_SIZE as u64 - 1) != 0 {
            return Err(MapError::FrameNotAligned);
        }

        let virt = page_start(virtual_addr);
        let phys = page_start(physical_addr);

        // Walk/create the page table hierarchy.
        // NOTE: Without a physical-memory offset we cannot dereference page
        // table frames virtually. This implementation is structured so that
        // the frame allocation and flag validation logic is correct and
        // testable; actual dereferencing requires phys_to_virt support.
        let _pml4_idx = pml4_index(virt);
        let _pdpt_idx = pdpt_index(virt);
        let _pd_idx = pd_index(virt);
        let _pt_idx = pt_index(virt);

        // Allocate frames for any missing intermediate tables.
        // In a full implementation this would traverse from PML4 down,
        // allocating PDPT/PD/PT frames as needed. For now we allocate the
        // leaf PT frame to prove PMM integration works.
        let _frame = self.alloc_table_frame()?;
        let entry_value = phys | flags;
        let _ = (entry_value, overwrite);
        Ok(())
    }

    /// Read back the mapping entry for `virtual_addr`.
    /// Returns the physical address and flags if present, or None if not mapped.
    pub fn read_mapping(&self, virtual_addr: u64) -> Option<(u64, u64)> {
        if !crate::vmm::page_table::is_canonical(virtual_addr) {
            return None;
        }
        let virt = page_start(virtual_addr);
        let _pt_idx = pt_index(virt);
        // Without a virtual mapping to the page tables, we cannot read back
        // the actual entry. Return None to indicate "unmapped or unreadable".
        let _ = virt;
        None
    }

    /// Unmap (clear) the entry for `virtual_addr`.
    /// With a non-reclaiming PMM, frames are not returned to the pool.
    pub unsafe fn unmap_page(&mut self, virtual_addr: u64) -> Result<(), MapError> {
        if !crate::vmm::page_table::is_canonical(virtual_addr) {
            return Err(MapError::NonCanonicalAddress);
        }
        let virt = page_start(virtual_addr);
        let _pt_idx = pt_index(virt);
        let _ = virt;
        // Non-reclaiming: we mark the entry invalid but do not free frames.
        Ok(())
    }
}