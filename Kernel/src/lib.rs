//! Kernel library (host-side tests only).

#![cfg_attr(not(test), no_std)]
pub mod serial;
pub mod pmm;
pub mod bootinfo;
pub mod memory;
pub mod exit;

pub mod vmm;

pub mod heap;
