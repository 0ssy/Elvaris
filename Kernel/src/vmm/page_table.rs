//! Page table structures.

#![allow(dead_code)]

use crate::vmm::entry::Flags;
use crate::pmm::phys_alloc::PhysFrameAllocator;

pub const PAGE_SIZE: u64 = 4096;
pub const ENTRIES_PER_TABLE: usize = 512;

#[repr(C, align(4096))]
#[derive(Clone)]
pub struct PageTable {
    entries: [u64; ENTRIES_PER_TABLE],
}

impl PageTable {
    pub fn new() -> Self {
        Self {
            entries: [0u64; ENTRIES_PER_TABLE],
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
    core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack));
    cr3 & !0xFFF // Clear lower 12 bits
}
