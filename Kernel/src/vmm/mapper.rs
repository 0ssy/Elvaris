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

/// Flags used for intermediate page-table entries: present and writable by the
/// kernel.
///
/// The entries of the upper levels of the hierarchy carry no no-execute bit:
/// such a bit does not describe the table itself, it describes every page
/// below the table, and it would make all of them non-executable. Whether a
/// page may be executed is decided by the flag of its own entry.
pub const TABLE_FLAGS: u64 = entry_flags::PRESENT | entry_flags::WRITABLE;

/// Flags used for the intermediate page-table entries of a user address space.
///
/// The user bit has to be set on every level of the hierarchy. The processor
/// checks it at each level on the way to the leaf: a user-mode access
/// through a table without the user bit raises a page fault even when the
/// page itself is accessible from user mode.
pub const USER_TABLE_FLAGS: u64 = TABLE_FLAGS | entry_flags::USER_ACCESSIBLE;

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
    /// The number of frames the mapper took from the physical-memory
    /// allocator since the last time the counter was taken.
    frames_allocated: u64,
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
        // Record the offset: the kernel needs it to dereference the page
        // tables from contexts that cannot borrow the mapper.
        crate::vmm::set_physical_memory_offset(offset);
        Ok(Self {
            allocator,
            pml4_phys: unsafe { current_pml4() },
            phys_offset: offset,
            frames_allocated: 0,
        })
    }

    pub fn pml4_address(&self) -> u64 {
        self.pml4_phys
    }

    /// Point the mapper at another top-level table and return the previous target.
    pub fn retarget(&mut self, pml4_phys: u64) -> u64 {
        let previous = self.pml4_phys;
        self.pml4_phys = pml4_phys;
        previous
    }

    /// Take a zeroed frame from the PMM for use as a page table.
    pub fn allocate_table_frame(&mut self) -> Result<u64, MapError> {
        let frame = self.allocate_frame()?;
        let table = unsafe { self.table_at(frame) };
        unsafe {
            core::ptr::write_bytes(table as *mut u8, 0, crate::vmm::page_table::PAGE_SIZE as usize);
        }
        Ok(frame)
    }

    /// Copy the 512 entries of the current PML4 into another PML4 frame.
    pub fn copy_pml4(&self, destination: u64) -> Result<(), MapError> {
        let src = unsafe { self.table_at(self.pml4_phys) };
        let dst = unsafe { self.table_at(destination) };
        unsafe {
            core::ptr::copy_nonoverlapping(src as *const u8, dst as *mut u8, crate::vmm::page_table::PAGE_SIZE as usize);
        }
        Ok(())
    }

    /// Hand out the frames the mapper took since the last call.
    pub fn copy_allocated_frames(&mut self, out: &mut [u64]) -> usize {
        // The existing counter only tracks the count, not individual frames,
        // so this reports how many were taken without enumerating them.
        let n = self.frames_allocated as usize;
        let to_copy = n.min(out.len());
        for slot in out.iter_mut().take(to_copy) {
            *slot = 0;
        }
        self.frames_allocated = 0;
        to_copy
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
            self.frames_allocated += 1;
            if frame != 0 {
                return Ok(frame);
            }
        }
    }

    /// The number of frames the mapper took from the physical-memory
    /// allocator since the last time the counter was taken.
    ///
    /// The counter is reset, so the caller can measure how many frames a
    /// single operation needed.
    pub fn take_allocated_frames(&mut self) -> u64 {
        let frames = self.frames_allocated;
        self.frames_allocated = 0;
        frames
    }

    /// Return the table at `index` inside `parent`, creating it if needed.
    ///
    /// A newly created table is zeroed and marked with `table_flags`. The
    /// flags of an existing entry are widened instead: the table flags are
    /// added to them, and the no-execute bit is removed. A table that the
    /// bootloader created for the kernel therefore becomes usable from user
    /// mode as soon as a user program needs it.
    ///
    /// The second element of the result reports whether the entry changed,
    /// which means an upper-level paging structure was modified and the TLB
    /// has to be flushed as a whole.
    fn next_table(
        &mut self,
        parent: *mut PageTable,
        index: usize,
        table_flags: u64,
    ) -> Result<(*mut PageTable, bool), MapError> {
        let entry = unsafe { (*parent).get_entry(index) };
        if entry & entry_flags::PRESENT != 0 {
            if entry & entry_flags::HUGE_PAGE != 0 {
                return Err(MapError::HugePageUnsupported);
            }
            let frame = entry & FRAME_MASK;

            // Keep the flags of the entry, drop its address, and apply the
            // flags the caller asked for. The frame of the table stays.
            let wanted = frame | ((entry & !FRAME_MASK & !entry_flags::NO_EXECUTE) | table_flags);
            if wanted != entry {
                unsafe { (*parent).set_entry(index, wanted) };
                return Ok((unsafe { self.table_at(frame) }, true));
            }
            Ok((unsafe { self.table_at(frame) }, false))
        } else {
            // Allocate the new table from the PMM.
            let frame = self.allocate_frame()?;
            let table = unsafe { self.table_at(frame) };
            // The frame is uninitialized memory: zero it before use.
            unsafe {
                core::ptr::write_bytes(table as *mut u8, 0, PAGE_SIZE as usize);
                (*parent).set_entry(index, frame | table_flags);
            }
            Ok((table, true))
        }
    }

    /// The flags the intermediate tables of a mapping carry.
    ///
    /// A page that is accessible from user mode needs tables that are
    /// accessible from user mode as well: the processor checks the user bit
    /// on every level of the hierarchy before it lets a user-mode access
    /// reach the page.
    fn table_flags_for(flags: u64) -> u64 {
        if flags & entry_flags::USER_ACCESSIBLE != 0 {
            USER_TABLE_FLAGS
        } else {
            TABLE_FLAGS
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
        let table_flags = Self::table_flags_for(flags);

        let pml4 = unsafe { self.table_at(self.pml4_phys) };
        let (pdpt, pdpt_changed) = self.next_table(pml4, pml4_index(virt), table_flags)?;
        let (pd, pd_changed) = self.next_table(pdpt, pdpt_index(virt), table_flags)?;
        let (pt, pt_changed) = self.next_table(pd, pd_index(virt), table_flags)?;

        let existing = unsafe { (*pt).get_entry(pt_index(virt)) };
        if existing & entry_flags::PRESENT != 0 {
            return Err(MapError::MappingAlreadyExists);
        }

        unsafe { (*pt).set_entry(pt_index(virt), phys | flags) };
        flush_tlb(virt);

        if pdpt_changed || pd_changed || pt_changed {
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
        crate::vmm::translate::translate(self.pml4_phys, self.phys_offset, virtual_addr)
    }

    /// Return the leaf page table that holds the entry for `virtual_addr`.
    ///
    /// Every level of the hierarchy has to be present, and the entries may
    /// not describe a huge page: the kernel maps 4 KiB pages only.
    fn leaf_table(&self, virt: u64) -> Result<*mut PageTable, MapError> {
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
        if pt_entry & entry_flags::HUGE_PAGE != 0 {
            return Err(MapError::HugePageUnsupported);
        }
        Ok(unsafe { self.table_at(pt_entry & FRAME_MASK) })
    }

    /// Replace the flags of an existing mapping.
    ///
    /// The physical frame stays the same; only the permission bits of the
    /// entry change. The kernel uses this to make a page read-only once the
    /// program is loaded into it.
    pub fn update_flags(&mut self, virtual_addr: u64, flags: u64) -> Result<(), MapError> {
        if !is_canonical(virtual_addr) {
            return Err(MapError::NonCanonicalAddress);
        }
        MapFlags::validate(flags)?;
        let virt = page_start(virtual_addr);

        let pt = self.leaf_table(virt)?;
        let pte = unsafe { (*pt).get_entry(pt_index(virt)) };
        if pte & entry_flags::PRESENT == 0 {
            return Err(MapError::NotMapped);
        }

        let frame = pte & FRAME_MASK;
        unsafe { (*pt).set_entry(pt_index(virt), frame | flags) };
        flush_tlb(virt);
        Ok(())
    }

    /// Unmap (clear) the entry for `virtual_addr` and return the frame.
    ///
    /// With a non-reclaiming PMM, the frame is not returned to the pool.
    pub fn unmap_page(&mut self, virtual_addr: u64) -> Result<u64, MapError> {
        if !is_canonical(virtual_addr) {
            return Err(MapError::NonCanonicalAddress);
        }
        let virt = page_start(virtual_addr);

        let pt = self.leaf_table(virt)?;

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
