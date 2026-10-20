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
