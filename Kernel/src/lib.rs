//! Kernel library (host-side tests only).

#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, no_std)]

#[cfg(any(test, feature = "alloc"))]
extern crate alloc;

pub mod serial;
pub mod pmm;
pub mod bootinfo;
pub mod memory;
pub mod exit;

pub mod vmm;

pub mod heap;

pub mod cpu;

#[cfg(all(feature = "alloc", not(test)))]
pub mod threads;


