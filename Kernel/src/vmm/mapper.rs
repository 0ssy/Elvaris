//! Page table mapping utilities.

#![allow(dead_code)]

use crate::pmm::phys_alloc::PhysFrameAllocator;
use crate::vmm::page_table::{current_pml4, PAGE_SIZE, ENTRIES_PER_TABLE};
use crate::vmm::entry::Flags;

pub struct Mapper<'a> {
    allocator: &'a mut PhysFrameAllocator,
    pml4_phys: u64,
}

impl<'a> Mapper<'a> {
    pub unsafe fn new(allocator: &'a mut PhysFrameAllocator) -> Self {
        Self {
            allocator,
            pml4_phys: current_pml4(),
        }
    }

    pub fn pml4_address(&self) -> u64 {
        self.pml4_phys
    }
}
