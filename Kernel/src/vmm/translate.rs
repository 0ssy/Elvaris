//! Address translation without a mapper.
//!
//! The mapper owns the physical-memory allocator and therefore cannot be
//! used from an interrupt handler that has to inspect the page tables, for
//! example while it validates a pointer a user program handed to the kernel.
//!
//! This module walks the page table hierarchy with nothing but the physical
//! address of the top-level table and the base of the physical-memory direct
//! map. It never allocates and never changes a mapping, so it is safe to use
//! from any context.

use crate::vmm::page_table::{
    current_pml4, entry_flags, is_canonical, page_start, pd_index, pdpt_index, pml4_index, pt_index,
    PageTable, FRAME_MASK,
};

/// The mask that keeps the permission bits of a page table entry.
///
/// The low twelve bits hold the permission bits of the entry, and bit 63
/// holds the no-execute bit. The bit 63 has to be part of the mask: an entry
/// that carries it describes a page the processor refuses to execute, and a
/// diagnostic that does not show it hides the reason for the fault.
const FLAGS_MASK: u64 = 0x8000_0000_0000_0FFF;

/// Translate a virtual address in the address space rooted at `pml4_phys`.
///
/// Returns the physical address of the frame and the flags of the page table
/// entry, or `None` when the address is not mapped. The flags are the
/// permission bits of the entry: the low twelve bits and the no-execute bit.
pub fn translate(pml4_phys: u64, phys_offset: u64, address: u64) -> Option<(u64, u64)> {
    if !is_canonical(address) {
        return None;
    }
    let virt = page_start(address);

    // The direct map makes every page table frame reachable at a known
    // virtual address.
    let table_at = |phys: u64| -> *mut PageTable { phys.wrapping_add(phys_offset) as *mut PageTable };

    // SAFETY: every table pointer is derived from a page table entry that was
    // checked to be present, and the direct map makes the frame reachable.
    unsafe {
        let pml4 = table_at(pml4_phys);

        let pdpt_entry = (*pml4).get_entry(pml4_index(virt));
        if pdpt_entry & entry_flags::PRESENT == 0 {
            return None;
        }
        let pdpt = table_at(pdpt_entry & FRAME_MASK);

        let pd_entry = (*pdpt).get_entry(pdpt_index(virt));
        if pd_entry & entry_flags::PRESENT == 0 {
            return None;
        }
        if pd_entry & entry_flags::HUGE_PAGE != 0 {
            return None;
        }
        let pd = table_at(pd_entry & FRAME_MASK);

        let pt_entry = (*pd).get_entry(pd_index(virt));
        if pt_entry & entry_flags::PRESENT == 0 {
            return None;
        }
        if pt_entry & entry_flags::HUGE_PAGE != 0 {
            return None;
        }
        let pt = table_at(pt_entry & FRAME_MASK);

        let pte = (*pt).get_entry(pt_index(virt));
        if pte & entry_flags::PRESENT == 0 {
            return None;
        }
        Some((pte & FRAME_MASK, pte & FLAGS_MASK))
    }
}

/// Translate a virtual address in the currently active address space.
///
/// Returns `None` when the kernel does not know the physical-memory direct
/// map, because the page tables cannot be dereferenced without it.
pub fn translate_current(address: u64) -> Option<(u64, u64)> {
    let offset = crate::vmm::physical_memory_offset();
    if offset == 0 {
        return None;
    }
    // SAFETY: the direct map is present in the active address space, so the
    // page tables of that address space are reachable.
    let pml4 = unsafe { current_pml4() };
    translate(pml4, offset, address)
}

/// The raw entries of the page-table walk for `address`.
///
/// The result holds the entry of the top-level table, of the second level, of
/// the third level and of the leaf table, in that order. An entry that is
/// zero belongs to a level that is not present, and the levels below it are
/// zero as well.
///
/// The function is the diagnostic counterpart of [`translate`]: it shows the
/// permission bits of every level, so a fault that comes from a level above
/// the leaf can be told apart from a fault that comes from the leaf itself.
pub fn walk(pml4_phys: u64, phys_offset: u64, address: u64) -> [u64; 4] {
    let mut entries = [0u64; 4];
    if !is_canonical(address) {
        return entries;
    }
    let virt = page_start(address);

    let table_at = |phys: u64| -> *mut PageTable { phys.wrapping_add(phys_offset) as *mut PageTable };

    // SAFETY: every table pointer is derived from a page table entry that was
    // checked to be present, and the direct map makes the frame reachable.
    unsafe {
        let pml4 = table_at(pml4_phys);

        entries[0] = (*pml4).get_entry(pml4_index(virt));
        if entries[0] & entry_flags::PRESENT == 0 {
            return entries;
        }
        let pdpt = table_at(entries[0] & FRAME_MASK);

        entries[1] = (*pdpt).get_entry(pdpt_index(virt));
        if entries[1] & entry_flags::PRESENT == 0 {
            return entries;
        }
        let pd = table_at(entries[1] & FRAME_MASK);

        entries[2] = (*pd).get_entry(pd_index(virt));
        if entries[2] & entry_flags::PRESENT == 0 {
            return entries;
        }
        let pt = table_at(entries[2] & FRAME_MASK);

        entries[3] = (*pt).get_entry(pt_index(virt));
    }

    entries
}

/// The raw entries of the page-table walk for `address` in the currently
/// active address space.
pub fn walk_current(address: u64) -> [u64; 4] {
    let offset = crate::vmm::physical_memory_offset();
    if offset == 0 {
        return [0; 4];
    }
    // SAFETY: the direct map is present in the active address space, so the
    // page tables of that address space are reachable.
    let pml4 = unsafe { current_pml4() };
    walk(pml4, offset, address)
}
