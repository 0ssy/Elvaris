//! Page table structures and x86-64 virtual address translation helpers.

#![allow(dead_code)]

use crate::pmm::phys_alloc::PhysFrameAllocator;

pub const PAGE_SIZE: u64 = 4096;
pub const ENTRIES_PER_TABLE: usize = 512;

/// Index bit positions for the four-level x86-64 page table walk (4 KiB pages).
const PML4_SHIFT: u64 = 39;
const PDPT_SHIFT: u64 = 30;
const PD_SHIFT: u64 = 21;
const PT_SHIFT: u64 = 12;
const ENTRY_MASK: usize = 512 - 1;

/// x86-64 page table entry flags (bits within the 64-bit entry).
pub mod entry_flags {
    pub const PRESENT: u64 = 1 << 0;
    pub const WRITABLE: u64 = 1 << 1;
    pub const USER_ACCESSIBLE: u64 = 1 << 2;
    pub const WRITE_THROUGH: u64 = 1 << 3;
    pub const NO_CACHE: u64 = 1 << 4;
    pub const ACCESSED: u64 = 1 << 5;
    pub const DIRTY: u64 = 1 << 6;
    pub const HUGE_PAGE: u64 = 1 << 7;
    pub const GLOBAL: u64 = 1 << 8;
    pub const NO_EXECUTE: u64 = 1 << 63;
}

/// Return the index into the PML4 table for the given virtual address.
pub fn pml4_index(addr: u64) -> usize {
    ((addr >> PML4_SHIFT) & ENTRY_MASK as u64) as usize
}

/// Return the index into the PDPT table for the given virtual address.
pub fn pdpt_index(addr: u64) -> usize {
    ((addr >> PDPT_SHIFT) & ENTRY_MASK as u64) as usize
}

/// Return the index into the Page Directory for the given virtual address.
pub fn pd_index(addr: u64) -> usize {
    ((addr >> PD_SHIFT) & ENTRY_MASK as u64) as usize
}

/// Return the index into the Page Table for the given virtual address.
pub fn pt_index(addr: u64) -> usize {
    ((addr >> PT_SHIFT) & ENTRY_MASK as u64) as usize
}

/// Return the page-aligned start address of the page containing `addr`.
pub fn page_start(addr: u64) -> u64 {
    addr & !(PAGE_SIZE - 1)
}

/// Return the offset within a page for the given address.
pub fn page_offset(addr: u64) -> u64 {
    addr & (PAGE_SIZE - 1)
}

/// Check whether a virtual address is canonical for x86-64 (sign-extended bit 47).
pub fn is_canonical(addr: u64) -> bool {
    // Bits 48-63 must all equal bit 47.
    let sign = (addr >> 47) & 1;
    if sign == 0 {
        addr >> 47 == 0
    } else {
        addr >> 47 == 0x1_FFFF
    }
}

#[repr(C, align(4096))]
#[derive(Clone)]
pub struct PageTable {
    entries: [u64; ENTRIES_PER_TABLE],
}

impl PageTable {
    pub fn new() -> Self {
        Self {
            entries: [0u64; 512],
        }
    }

    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            *entry = 0;
        }
    }

    pub fn get_entry(&self, index: usize) -> u64 {
        self.entries[index]
    }

    pub fn set_entry(&mut self, index: usize, value: u64) {
        self.entries[index] = value;
    }

    pub fn is_present(&self, index: usize) -> bool {
        (self.entries[index] & entry_flags::PRESENT) != 0
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the physical address of the current PML4 (CR3).
#[inline]
pub unsafe fn current_pml4() -> u64 {
    let cr3: u64;
    unsafe { core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack)); }
    cr3 & !0xFFF // Clear lower 12 bits
}