//! Physical frame allocator (simple bump allocator for milestone 3 foundation).
//!
//! Tracks usable physical memory frames and supports allocation. This is a
//! non-reclaiming (bump) allocator - freed frames are not returned to the pool.
//! Deallocation is not yet implemented; this limitation is documented and will
//! be addressed when switching to a bitmap-based allocator.

use crate::pmm::frame::FRAME_SIZE;
use bootloader_api::info::{MemoryRegion, MemoryRegionKind};

const MAX_REGIONS: usize = 256;

#[derive(Debug, Clone, Copy)]
struct ManagedRegion {
    start: u64,
    end: u64, // exclusive, frame-aligned
    cursor: u64,
}

/// Physical frame allocator managing usable memory regions.
///
/// # Safety Invariants
/// 1. Every allocated frame is 4 KiB aligned.
/// 2. Every allocated frame is within a bootloader-marked usable region.
/// 3. No frame is allocated twice (enforced by bump cursor).
/// 4. The allocator never hands out memory that overlaps kernel/bootloader/
///    firmware-reserved structures (only allocates from Usable regions).
/// 5. Region arithmetic is checked (alignments computed carefully).
/// 6. Unknown memory is never treated as free.
/// 7. Partial frames are never returned.
///
/// # Limitations
/// - Non-reclaiming: once allocated, frames cannot be freed (bump allocator).
/// - Single-threaded: no synchronization primitives; requires external locking
///   if used across multiple cores/threads.
#[derive(Debug)]
pub struct PhysFrameAllocator {
    regions: [ManagedRegion; MAX_REGIONS],
    count: usize,
    total_frames: u64,
    allocated_frames: u64,
}

impl PhysFrameAllocator {
    pub fn new() -> Self {
        Self {
            regions: [ManagedRegion { start: 0, end: 0, cursor: 0 }; MAX_REGIONS],
            count: 0,
            total_frames: 0,
            allocated_frames: 0,
        }
    }

    /// Initialize the allocator from the bootloader's memory map.
    ///
    /// Only MemoryRegionKind::Usable regions are considered. Regions are
    /// normalized to frame-aligned boundaries; partial trailing frames are
    /// dropped. Returns true if initialization succeeded.
    pub fn init(&mut self, regions: &[MemoryRegion]) -> bool {
        self.count = 0;
        self.total_frames = 0;
        self.allocated_frames = 0;

        for r in regions {
            if self.count >= MAX_REGIONS {
                return false;
            }
            if r.kind != MemoryRegionKind::Usable {
                continue;
            }
            if r.start >= r.end {
                continue;
            }
            let aligned_start = ((r.start + FRAME_SIZE - 1) / FRAME_SIZE) * FRAME_SIZE;
            let aligned_end = (r.end / FRAME_SIZE) * FRAME_SIZE;
            if aligned_end <= aligned_start {
                continue;
            }
            let frames = (aligned_end - aligned_start) / FRAME_SIZE;
            if frames == 0 {
                continue;
            }
            self.regions[self.count] = ManagedRegion {
                start: aligned_start,
                end: aligned_end,
                cursor: aligned_start,
            };
            self.count += 1;
            self.total_frames += frames;
        }
        true
    }

    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    pub fn allocated_frames(&self) -> u64 {
        self.allocated_frames
    }

    pub fn free_frames(&self) -> u64 {
        self.total_frames.saturating_sub(self.allocated_frames)
    }

    pub fn usable_region_count(&self) -> usize {
        self.count
    }

    /// Allocate a physical frame. Returns the physical address of the frame,
    /// or None if no frames are available (exhaustion).
    ///
    /// Allocated frames are 4 KiB aligned and never double-allocated.
    pub fn allocate_frame(&mut self) -> Option<u64> {
        for i in 0..self.count {
            let r = &mut self.regions[i];
            if r.cursor + FRAME_SIZE <= r.end {
                let addr = r.cursor;
                r.cursor += FRAME_SIZE;
                self.allocated_frames += 1;
                return Some(addr);
            }
        }
        None
    }

    /// Allocate a run of consecutive physical frames.
    ///
    /// The function returns the physical address of the first frame of the
    /// run, or `None` when no region is big enough. A device that reaches
    /// the memory of the kernel directly needs such a run: the parts of a
    /// request have to lie one behind the other.
    pub fn allocate_contiguous_frames(&mut self, count: usize) -> Option<u64> {
        if count == 0 {
            return None;
        }
        let needed = (count as u64) * FRAME_SIZE;
        for i in 0..self.count {
            let r = &mut self.regions[i];
            if r.cursor + needed <= r.end {
                let addr = r.cursor;
                r.cursor += needed;
                self.allocated_frames += count as u64;
                return Some(addr);
            }
        }
        None
    }

    /// Deallocate a physical frame.
    ///
    /// # Limitation
    /// This bump allocator does not support deallocation. Calling this
    /// method is a no-op and is provided for API compatibility; it will
    /// be replaced with proper free semantics in a future bitmap allocator.
    #[allow(dead_code)]
    pub fn deallocate_frame(&mut self, _addr: u64) {
        // Non-reclaiming allocator - no-op
    }
}

impl Default for PhysFrameAllocator {
    fn default() -> Self {
        Self::new()
    }
}
