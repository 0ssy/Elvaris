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
pub mod security;
pub mod net;

pub mod console;
pub mod input;pub mod storage;
pub mod sysinfo;

#[cfg(any(test, feature = "alloc"))]
pub mod threads;

pub mod block;

pub mod fs;

pub mod pci;
pub mod device;

#[cfg(feature = "usb")]
pub mod usb;

#[cfg(feature = "audio")]
pub mod audio;

#[cfg(feature = "audio")]
pub mod audio_driver;

pub mod power;
pub mod sleep;
pub mod hotplug;
pub mod devicetree;
pub mod virtio;

#[cfg(feature = "packages")]
pub mod packages;

#[cfg(feature = "graphics")]
pub mod graphics;

#[cfg(feature = "graphics")]
pub mod surface;

#[cfg(feature = "graphics")]
pub mod render;

#[cfg(feature = "graphics")]
pub mod graphics_input;

#[cfg(feature = "graphics")]
pub mod compositor;

#[cfg(feature = "graphics")]
pub mod window;

#[cfg(feature = "graphics")]
pub mod graphics_session;

#[cfg(feature = "shell")]
pub mod terminal_emulator;

#[cfg(feature = "desktop")]
pub mod desktop;

#[cfg(feature = "graphics")]
pub mod graphics_test;
