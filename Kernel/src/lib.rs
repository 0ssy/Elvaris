//! Kernel library (host-side tests only).

#![cfg_attr(not(test), no_std)]

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

pub mod user;
pub mod process;

#[cfg(any(test, feature = "alloc"))]
pub mod threads;


