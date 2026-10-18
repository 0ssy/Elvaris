//! Physical frame representation and utilities (4 KiB frames).

use bootloader_api::info::MemoryRegion;

/// Size of a physical memory frame in bytes.
pub const FRAME_SIZE: u64 = 4096;

/// A physical memory frame identified by its start address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    start_addr: u64,
}

impl Frame {
    /// Create a new frame from a physical address. The address must be
    /// frame-aligned.
    pub fn containing_address(addr: u64) -> Self {
        Self {
            start_addr: addr & !(FRAME_SIZE - 1),
        }
    }

    /// Get the start address of this frame.
    pub fn start_address(&self) -> u64 {
        self.start_addr
    }

    /// Get the end address (exclusive) of this frame.
    pub fn end_address(&self) -> u64 {
        self.start_addr + FRAME_SIZE
    }

    pub fn from_address(addr: u64) -> Self {
        Self::containing_address(addr)
    }
}

/// Normalize a usable region into a range of frame-aligned addresses.
/// Returns (start_frame_addr, end_frame_addr_exclusive) aligned to frame size.
/// Drops partial trailing frames.
pub fn normalize_region(region: &MemoryRegion) -> Option<(u64, u64)> {
    if region.start >= region.end {
        return None;
    }
    let aligned_start = ((region.start + FRAME_SIZE - 1) / FRAME_SIZE) * FRAME_SIZE;
    let aligned_end = (region.end / FRAME_SIZE) * FRAME_SIZE;
    if aligned_end <= aligned_start {
        return None;
    }
    Some((aligned_start, aligned_end))
}

/// Count how many full frames fit in the given byte range.
pub fn frame_count(bytes: u64) -> u64 {
    bytes / FRAME_SIZE
}
