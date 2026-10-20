//! Page table mapping utilities.
//!
//! The mapper walks the four-level page table hierarchy to establish 4 KiB
//! mappings. Page-table frames are allocated from the PMM, and mapped frames
//! are backed by PMM frames as well.
//!
//! # Access to page tables
//!
//! The mapper reaches page-table frames through the physical-memory direct map
//! that the bootloader sets up (see `BootInfo::physical_memory_offset`). The
//! kernel requests that mapping through its `.bootloader-config` section; if it
//! is missing, the mapper cannot be created and the kernel refuses to
//! initialize the heap rather than silently working with an unusable address
//! space.
//!
//! # Limitations
//!
//! Because the PMM is a non-reclaiming bump allocator, unmapping a page does
//! not return the frame to the PMM. This limitation is documented and visible
//! in the PMM statistics.

#![allow(dead_code)]

use crate::pmm::phys_alloc::PhysFrameAllocator;
use crate::vmm::page_table::entry_flags;
use crate::vmm::page_table::{
    current_pml4, is_canonical, page_start, pd_index, pdpt_index, pml4_index, pt_index, PageTable,
    FRAME_MASK, PAGE_SIZE,
};

/// Flags used for intermediate page-table entries: present, writable by the
/// kernel, and never executable.
pub const TABLE_FLAGS: u64 = entry_flags::PRESENT | entry_flags::WRITABLE | entry_flags::NO_EXECUTE;

/// Error returned by mapping operations.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MapError {
    /// The requested virtual address is not canonical.
    NonCanonicalAddress,
    /// The page table entry already exists.
    MappingAlreadyExists,
    /// A page table entry exists but is not a table where one is needed.
    NotATable,
    /// The physical address is not page aligned.
    FrameNotAligned,
    /// No page-table frames available from the allocator.
    OutOfMemory,
    /// The address is covered by a huge page, which this mapper cannot split.
    HugePageUnsupported,
    /// The address is not mapped.
    NotMapped,
    /// The bootloader did not provide a physical-memory mapping, so page
    /// tables cannot be dereferenced.
    NoPhysicalMemoryMapping,
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
        // Huge pages are not supported by this mapper.
        if (bits & entry_flags::HUGE_PAGE) != 0 {
            return Err(MapError::HugePageUnsupported);
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

    /// Flags for a page-table entry that describes a table (not a leaf page).
    pub const fn table() -> Self {
        Self(TABLE_FLAGS)
    }
}

/// Mapper walks the page table hierarchy using physical addresses.
///
/// The mapper borrows the PMM mutably: every page-table frame and every mapped
/// frame is obtained from it.
pub struct Mapper<'a> {
    allocator: &'a mut PhysFrameAllocator,
    pml4_phys: u64,
    phys_offset: u64,
}

impl<'a> Mapper<'a> {
    /// Create a mapper for the currently active address space.
    ///
    /// `phys_offset` is the physical-memory direct map base reported by the
    /// bootloader in `BootInfo::physical_memory_offset`. It is required: the
    /// kernel's page tables are not identity mapped, so without the direct map
    /// the page tables cannot be dereferenced at all.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the currently loaded CR3 belongs to the
    /// address space the kernel intends to manage, and that the direct map
    /// described by `phys_offset` is present in that address space.
    pub unsafe fn new(
        allocator: &'a mut PhysFrameAllocator,
        phys_offset: Option<u64>,
    ) -> Result<Self, MapError> {
        let offset = phys_offset.ok_or(MapError::NoPhysicalMemoryMapping)?;
        Ok(Self {
            allocator,
            pml4_phys: unsafe { current_pml4() },
            phys_offset: offset,
        })
    }

    pub fn pml4_address(&self) -> u64 {
        self.pml4_phys
    }

    /// Base address of the physical-memory direct map.
    pub fn physical_memory_offset(&self) -> u64 {
        self.phys_offset
    }

    /// Read the PML4 entry at `index`.
    ///
    /// A present entry means that the whole 512 GiB region covered by that
    /// entry is in use by some mapping.
    pub fn pml4_entry(&self, index: usize) -> u64 {
        let pml4 = unsafe { self.table_at(self.pml4_phys) };
        unsafe { (*pml4).get_entry(index) }
    }

    /// Whether the PML4 entry at `index` is used by any mapping.
    pub fn pml4_is_used(&self, index: usize) -> bool {
        self.pml4_entry(index) & entry_flags::PRESENT != 0
    }

    /// Translate a physical address into the direct map's virtual address.
    pub fn phys_to_virt(&self, phys: u64) -> u64 {
        phys.wrapping_add(self.phys_offset)
    }

    /// Obtain a pointer to a page-table frame.
    ///
    /// # Safety
    ///
    /// `phys` must be the physical address of a page-table frame that is
    /// mapped through the direct map.
    unsafe fn table_at(&self, phys: u64) -> *mut PageTable {
        self.phys_to_virt(phys) as *mut PageTable
    }

    /// Take a frame from the PMM.
    ///
    /// The frame at physical address zero is never used: the bootloader
    /// reserves it, and the Rust core library assumes that address zero is
    /// not a valid pointer, so mapping it would turn a null-pointer bug into
    /// silent memory corruption. The PMM cannot reclaim frames, so a skipped
    /// frame is wasted.
    fn allocate_frame(&mut self) -> Result<u64, MapError> {
        loop {
            let frame = self
                .allocator
                .allocate_frame()
                .ok_or(MapError::OutOfMemory)?;
            if frame != 0 {
                return Ok(frame);
            }
        }
    }

    /// Return the table at `index` inside `parent`, creating it if needed.
    ///
    /// A newly created table is zeroed and marked present and writable. The
    /// second element of the result reports whether a table was created, which
    /// means an upper-level paging structure was modified and the TLB has to
    /// be flushed as a whole.
    fn next_table(
        &mut self,
        parent: *mut PageTable,
        index: usize,
    ) -> Result<(*mut PageTable, bool), MapError> {
        let entry = unsafe { (*parent).get_entry(index) };
        if entry & entry_flags::PRESENT != 0 {
            if entry & entry_flags::HUGE_PAGE != 0 {
                return Err(MapError::HugePageUnsupported);
            }
            let frame = entry & FRAME_MASK;
            Ok((unsafe { self.table_at(frame) }, false))
        } else {
            // Allocate the new table from the PMM.
            let frame = self.allocate_frame()?;
            let table = unsafe { self.table_at(frame) };
            // The frame is uninitialized memory: zero it before use.
            unsafe {
                core::ptr::write_bytes(table as *mut u8, 0, PAGE_SIZE as usize);
                (*parent).set_entry(index, frame | TABLE_FLAGS);
            }
            Ok((table, true))
        }
    }

    /// Map a single 4 KiB page: `virtual_addr` -> `physical_addr` with `flags`.
    ///
    /// Returns an error if the mapping already exists.
    pub fn map_page_to(
        &mut self,
        virtual_addr: u64,
        physical_addr: u64,
        flags: u64,
    ) -> Result<(), MapError> {
        if !is_canonical(virtual_addr) {
            return Err(MapError::NonCanonicalAddress);
        }
        MapFlags::validate(flags)?;
        if physical_addr & (PAGE_SIZE as u64 - 1) != 0 {
            return Err(MapError::FrameNotAligned);
        }

        let virt = page_start(virtual_addr);
        let phys = physical_addr & !(PAGE_SIZE as u64 - 1);

        let pml4 = unsafe { self.table_at(self.pml4_phys) };
        let (pdpt, pdpt_created) = self.next_table(pml4, pml4_index(virt))?;
        let (pd, pd_created) = self.next_table(pdpt, pdpt_index(virt))?;
        let (pt, pt_created) = self.next_table(pd, pd_index(virt))?;

        let existing = unsafe { (*pt).get_entry(pt_index(virt)) };
        if existing & entry_flags::PRESENT != 0 {
            return Err(MapError::MappingAlreadyExists);
        }

        unsafe { (*pt).set_entry(pt_index(virt), phys | flags) };
        flush_tlb(virt);

        if pdpt_created || pd_created || pt_created {
            // An upper-level paging structure was modified. The processor may
            // have cached the previous "not present" state, and a targeted
            // invalidation is not guaranteed to reach those caches, so the
            // whole TLB is flushed by reloading CR3.
            reload_cr3();
        }
        Ok(())
    }

    /// Map a single 4 KiB page to a freshly allocated physical frame.
    ///
    /// The frame is taken from the PMM and returned so the caller can report
    /// which physical memory backs the mapping.
    ///
    /// The frame at physical address zero is never mapped: the bootloader
    /// reserves it, and the Rust core library assumes that address zero is
    /// not a valid pointer, so mapping it would turn a null-pointer bug into
    /// silent memory corruption.
    pub fn map_page(&mut self, virtual_addr: u64, flags: u64) -> Result<u64, MapError> {
        let frame = self.allocate_frame()?;
        self.map_page_to(virtual_addr, frame, flags)?;
        Ok(frame)
    }

    /// Map `pages` consecutive 4 KiB pages starting at `virtual_addr`.
    ///
    /// If a mapping fails, the pages that were already mapped are unmapped
    /// again and the original error is returned. The frames backing them are
    /// not returned to the PMM (the PMM cannot reclaim frames).
    pub fn map_range(
        &mut self,
        virtual_addr: u64,
        pages: usize,
        flags: u64,
    ) -> Result<(), MapError> {
        let mut mapped = 0usize;
        for index in 0..pages {
            let virt = virtual_addr
                .checked_add((index as u64) * PAGE_SIZE)
                .ok_or(MapError::NonCanonicalAddress)?;
            match self.map_page(virt, flags) {
                Ok(_) => mapped += 1,
                Err(err) => {
                    for done in 0..mapped {
                        let undo = virtual_addr + (done as u64) * PAGE_SIZE;
                        let _ = self.unmap_page(undo);
                    }
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    /// Read back the mapping entry for `virtual_addr`.
    ///
    /// Returns the physical address and flags if present, or None if not mapped.
    pub fn translate(&self, virtual_addr: u64) -> Option<(u64, u64)> {
        if !is_canonical(virtual_addr) {
            return None;
        }
        let virt = page_start(virtual_addr);

        let pml4 = unsafe { self.table_at(self.pml4_phys) };
        let pdpt_entry = unsafe { (*pml4).get_entry(pml4_index(virt)) };
        if pdpt_entry & entry_flags::PRESENT == 0 {
            return None;
        }
        let pdpt = unsafe { self.table_at(pdpt_entry & FRAME_MASK) };

        let pd_entry = unsafe { (*pdpt).get_entry(pdpt_index(virt)) };
        if pd_entry & entry_flags::PRESENT == 0 {
            return None;
        }
        if pd_entry & entry_flags::HUGE_PAGE != 0 {
            return None;
        }
        let pd = unsafe { self.table_at(pd_entry & FRAME_MASK) };

        let pt_entry = unsafe { (*pd).get_entry(pd_index(virt)) };
        if pt_entry & entry_flags::PRESENT == 0 {
            return None;
        }
        if pt_entry & entry_flags::HUGE_PAGE != 0 {
            return None;
        }
        let pt = unsafe { self.table_at(pt_entry & FRAME_MASK) };

        let pte = unsafe { (*pt).get_entry(pt_index(virt)) };
        if pte & entry_flags::PRESENT == 0 {
            return None;
        }
        Some((
            pte & FRAME_MASK,
            pte & (PAGE_SIZE as u64 - 1),
        ))
    }

    /// Unmap (clear) the entry for `virtual_addr` and return the frame.
    ///
    /// With a non-reclaiming PMM, the frame is not returned to the pool.
    pub fn unmap_page(&mut self, virtual_addr: u64) -> Result<u64, MapError> {
        if !is_canonical(virtual_addr) {
            return Err(MapError::NonCanonicalAddress);
        }
        let virt = page_start(virtual_addr);

        let pml4 = unsafe { self.table_at(self.pml4_phys) };
        let pdpt_entry = unsafe { (*pml4).get_entry(pml4_index(virt)) };
        if pdpt_entry & entry_flags::PRESENT == 0 {
            return Err(MapError::NotMapped);
        }
        let pdpt = unsafe { self.table_at(pdpt_entry & FRAME_MASK) };

        let pd_entry = unsafe { (*pdpt).get_entry(pdpt_index(virt)) };
        if pd_entry & entry_flags::PRESENT == 0 {
            return Err(MapError::NotMapped);
        }
        if pd_entry & entry_flags::HUGE_PAGE != 0 {
            return Err(MapError::HugePageUnsupported);
        }
        let pd = unsafe { self.table_at(pd_entry & FRAME_MASK) };

        let pt_entry = unsafe { (*pd).get_entry(pd_index(virt)) };
        if pt_entry & entry_flags::PRESENT == 0 {
            return Err(MapError::NotMapped);
        }
        let pt = unsafe { self.table_at(pt_entry & FRAME_MASK) };

        let pte = unsafe { (*pt).get_entry(pt_index(virt)) };
        if pte & entry_flags::PRESENT == 0 {
            return Err(MapError::NotMapped);
        }

        let frame = pte & FRAME_MASK;
        unsafe { (*pt).set_entry(pt_index(virt), 0) };
        flush_tlb(virt);
        Ok(frame)
    }
}

/// Invalidate the TLB entry for a virtual address.
fn flush_tlb(virtual_addr: u64) {
    unsafe {
        core::arch::asm!(
            "invlpg [{0}]",
            in(reg) virtual_addr,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Flush the whole TLB by reloading CR3.
///
/// Reloading CR3 with the same value invalidates every non-global TLB entry
/// and the paging-structure caches that belong to the current address space.
fn reload_cr3() {
    unsafe {
        let cr3: u64;
        core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
        core::arch::asm!("mov cr3, {}", in(reg) cr3, options(nomem, nostack, preserves_flags));
    }
}
