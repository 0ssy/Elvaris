//! Virtual memory management.
//!
//! This module handles x86-64 page tables and virtual address space management.
//! It provides the page table structures and flags, an inspector for the
//! bootloader's paging state, page-fault diagnostics, and the [`mapper`],
//! which walks the four-level page table hierarchy to establish 4 KiB
//! mappings. Page-table frames and mapped frames both come from the PMM, so
//! the virtual-memory subsystem never manages physical memory itself.

#![allow(dead_code)]

pub mod entry;

pub mod page_table;

pub mod addr;

#[cfg(test)]
mod tests;

pub mod fault;

pub mod mapper;

pub mod translate;

use core::sync::atomic::{AtomicU64, Ordering};

/// The base of the physical-memory direct map of the active address space.
///
/// The kernel needs it to dereference its own page tables. It is recorded
/// when the mapper is created, because the mapper is the only component that
/// receives it from the boot information.
static PHYSICAL_MEMORY_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Record the base of the physical-memory direct map.
pub fn set_physical_memory_offset(offset: u64) {
    PHYSICAL_MEMORY_OFFSET.store(offset, Ordering::Relaxed);
}

/// The base of the physical-memory direct map.
///
/// Zero means the kernel does not know the offset, and the page tables
/// cannot be dereferenced.
pub fn physical_memory_offset() -> u64 {
    PHYSICAL_MEMORY_OFFSET.load(Ordering::Relaxed)
}
