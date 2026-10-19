//! Page table structures.

#![allow(dead_code)]

use core::ptr::NonNull;
use crate::vmm::entry::Flags;

pub const PAGE_SIZE: u64 = 4096;
pub const ENTRIES_PER_TABLE: usize = 512;
