//! The information the kernel reports about the machine.
//!
//! The kernel keeps the numbers of the physical memory outside of the
//! physical memory manager: the manager itself lives in the boot sequence,
//! where it borrows the frame allocator of the mapper, and the shell asks
//! the kernel for the state of the memory long after that.
//!
//! The report is the answer to the `mem` command of the shell.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::serial;

/// The number of frames the physical memory manager has.
static TOTAL_FRAMES: AtomicU64 = AtomicU64::new(0);

/// The number of frames that are in use.
static USED_FRAMES: AtomicU64 = AtomicU64::new(0);

/// The number of frames that are free.
static FREE_FRAMES: AtomicU64 = AtomicU64::new(0);

/// Record the state of the physical memory.
///
/// The kernel calls the function once, after the physical memory manager
/// has taken its frames.
pub fn record_frames(total: u64, used: u64, free: u64) {
    TOTAL_FRAMES.store(total, Ordering::Relaxed);
    USED_FRAMES.store(used, Ordering::Relaxed);
    FREE_FRAMES.store(free, Ordering::Relaxed);
}

/// The number of frames the physical memory manager has.
pub fn total_frames() -> u64 {
    TOTAL_FRAMES.load(Ordering::Relaxed)
}

/// The number of frames that are in use.
pub fn used_frames() -> u64 {
    USED_FRAMES.load(Ordering::Relaxed)
}

/// The number of frames that are free.
pub fn free_frames() -> u64 {
    FREE_FRAMES.load(Ordering::Relaxed)
}

/// Report the memory of the machine on the console.
///
/// The report holds the state of the physical memory and the state of the
/// kernel heap.
pub fn report_memory() {
    let heap = crate::heap::stats();
    let arena = match heap {
        Ok(stats) => stats.arena,
        Err(_) => return,
    };
    unsafe {
        serial::write_str("Elvaris kernel: mem: frames total=");
        serial::write_fmt(core::format_args!("{}", total_frames()));
        serial::write_str(" used=");
        serial::write_fmt(core::format_args!("{}", used_frames()));
        serial::write_str(" free=");
        serial::write_fmt(core::format_args!("{}", free_frames()));
        serial::write_newline();
        serial::write_str("Elvaris kernel: mem: heap capacity=");
        serial::write_fmt(core::format_args!("{}", arena.capacity));
        serial::write_str(" used=");
        serial::write_fmt(core::format_args!("{}", arena.used_bytes));
        serial::write_str(" free=");
        serial::write_fmt(core::format_args!("{}", arena.free_bytes));
        serial::write_newline();
        serial::write_str("Elvaris kernel: mem: heap blocks=");
        serial::write_fmt(core::format_args!("{}", arena.block_count));
        serial::write_str(" allocations=");
        serial::write_fmt(core::format_args!("{}", arena.allocations));
        serial::write_str(" frees=");
        serial::write_fmt(core::format_args!("{}", arena.frees));
        serial::write_newline();
    }
}
