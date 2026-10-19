//! Virtual memory management.
//!
//! This module handles x86-64 page tables and virtual address space management.
//! For Milestone 4, we focus on inspecting the bootloader's paging state,
//! defining page table structures and flags, and setting up the foundation
//! for safe virtual memory operations.

#![cfg_attr(not(test), no_std)]
#![allow(dead_code)]

pub mod entry;

pub mod page_table;

pub mod addr;

#[cfg(test)]
mod tests;

pub mod fault;

pub mod mapper;
