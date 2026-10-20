//! Kernel library (host-side tests only).
//!
//! The library exposes the kernel modules to the host test build. The bare
//! kernel binary has its own module tree in `main.rs`; the two have to stay
//! in step, because the modules that are gated here are compiled into the
//! binary without the gate.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod bootinfo;
pub mod exit;
pub mod memory;
pub mod pmm;
pub mod serial;

pub mod vmm;

pub mod heap;

pub mod cpu;

pub mod process;
pub mod user;

pub mod console;
pub mod storage;
pub mod sysinfo;

#[cfg(any(test, feature = "alloc"))]
pub mod threads;

pub mod block;

pub mod fs;
