//! Physical frame allocator (bitmap-based for reclaimability).
//!
//! Tracks usable physical memory frames and supports allocation and
//! release. The bitmap allocator tracks which frames are free/allocated,
//! respects reserved regions, and rejects double-releases.

extern crate alloc;

use crate::pmm::frame::FRAME_SIZE;
use bootloader_api::info::{MemoryRegion, MemoryRegionKind};

const MAX_REGIONS: usize = 256;
/// Maximum number of u64 words in the frame bitmap (3072 words = up to 768 MiB RAM).
const BITMAP_MAX_WORDS: usize = 3072;
/// Maximum number of reserved frame addresses tracked.
const RESERVED_MAX_FRAMES: usize = 256;

#[derive(Debug, Clone, Copy)]
struct ManagedRegion {
    start: u64,
    end: u64, // exclusive, frame-aligned
    frame_count: u64,
}

/// Physical frame allocator managing usable memory regions.
///
/// # Safety Invariants
/// 1. Every allocated frame is 4 KiB aligned.
/// 2. Every allocated frame is within a bootloader-marked usable region.
/// 3. No frame is allocated twice.
/// 4. The allocator never hands out memory that overlaps kernel/bootloader/
///    firmware-reserved structures (only allocates from Usable regions).
/// 5. Region arithmetic is checked (alignments computed carefully).
/// 6. Unknown memory is never treated as free.
/// 7. Partial frames are never returned.
#[derive(Debug)]
pub struct PhysFrameAllocator {
    regions: [ManagedRegion; MAX_REGIONS],
    count: usize,
    total_frames: u64,
    allocated_frames: u64,
    /// Bitmap tracking free/allocated frames across all regions
    bitmap: [u64; BITMAP_MAX_WORDS],
    /// Number of valid words in `bitmap`.
    bitmap_words: usize,
    /// Reserved frame addresses that must never be released
    reserved: [u64; RESERVED_MAX_FRAMES],
    /// Number of valid entries in `reserved`.
    reserved_count: usize,
}

impl PhysFrameAllocator {
    pub fn new() -> Self {
        Self {
            regions: [ManagedRegion { start: 0, end: 0, frame_count: 0 }; MAX_REGIONS],
            count: 0,
            total_frames: 0,
            allocated_frames: 0,
            bitmap: [0u64; BITMAP_MAX_WORDS],
            bitmap_words: 0,
            reserved: [0u64; RESERVED_MAX_FRAMES],
            reserved_count: 0,
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
        self.bitmap_words = 0;
        self.reserved_count = 0;

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
                frame_count: frames,
            };
            self.count += 1;
            self.total_frames += frames;
        }

        // Initialize bitmap: all frames free (bit set = free)
        let bitmap_words = ((self.total_frames as usize) + 63) / 64;
        if bitmap_words > BITMAP_MAX_WORDS {
            return false;
        }
        for i in 0..bitmap_words {
            self.bitmap[i] = u64::MAX;
        }
        self.bitmap_words = bitmap_words;
        // If total_frames not multiple of 64, mask off extra bits
        if self.total_frames % 64 != 0 {
            let extra_bits = (self.total_frames % 64) as u32;
            let last_word = bitmap_words - 1;
            self.bitmap[last_word] = (1u64 << extra_bits) - 1;
        }

        true
    }

    /// Mark a frame as reserved (never allocated and never released).
    pub fn reserve_frame(&mut self, addr: u64) {
        if !self.is_valid_frame(addr) {
            return;
        }
        let frame_idx = self.addr_to_index(addr);
        if frame_idx >= self.total_frames {
            return;
        }
        // Mark as allocated/reserved (clear bit)
        let word = (frame_idx as usize) / 64;
        let bit = (frame_idx as usize) % 64;
        self.bitmap[word] &= !(1u64 << bit);
        if !self.is_reserved(addr) && self.reserved_count < RESERVED_MAX_FRAMES {
            self.reserved[self.reserved_count] = addr;
            self.reserved_count += 1;
        }
    }

    fn is_reserved(&self, addr: u64) -> bool {
        for i in 0..self.reserved_count {
            if self.reserved[i] == addr {
                return true;
            }
        }
        false
    }

    fn is_valid_frame(&self, addr: u64) -> bool {
        if addr % FRAME_SIZE != 0 {
            return false;
        }
        for i in 0..self.count {
            let r = &self.regions[i];
            if addr >= r.start && addr < r.end {
                return true;
            }
        }
        false
    }

    fn addr_to_index(&self, addr: u64) -> u64 {
        let mut idx = 0u64;
        for i in 0..self.count {
            let r = &self.regions[i];
            if addr >= r.start && addr < r.end {
                return idx + (addr - r.start) / FRAME_SIZE;
            }
            idx += r.frame_count;
        }
        u64::MAX
    }

    fn index_to_addr(&self, mut idx: u64) -> Option<u64> {
        for i in 0..self.count {
            let r = &self.regions[i];
            if idx < r.frame_count {
                return Some(r.start + idx * FRAME_SIZE);
            }
            idx -= r.frame_count;
        }
        None
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
    pub fn allocate_frame(&mut self) -> Option<u64> {
        for i in 0..self.total_frames {
            let word = (i as usize) / 64;
            let bit = (i as usize) % 64;
            if word >= self.bitmap_words {
                break;
            }
            if (self.bitmap[word] & (1u64 << bit)) != 0 {
                // Frame is free, allocate it
                self.bitmap[word] &= !(1u64 << bit);
                self.allocated_frames += 1;
                return self.index_to_addr(i);
            }
        }
        None
    }

    /// Allocate a run of consecutive physical frames.
    pub fn allocate_contiguous_frames(&mut self, count: usize) -> Option<u64> {
        if count == 0 {
            return None;
        }
        let count_u64 = count as u64;
        // Try to find consecutive free frames within a single region
        for i in 0..self.count {
            let r = &self.regions[i];
            if r.frame_count < count_u64 {
                continue;
            }
            // Check if there are count_u64 consecutive free frames starting at some position in this region
            let start_idx = self.get_region_start_index(i);
            for offset in 0..=(r.frame_count - count_u64) {
                let mut all_free = true;
                for j in 0..count_u64 {
                    let idx = start_idx + offset + j;
                    let word = (idx as usize) / 64;
                    let bit = (idx as usize) % 64;
                    if word >= self.bitmap_words || (self.bitmap[word] & (1u64 << bit)) == 0 {
                        all_free = false;
                        break;
                    }
                }
                if all_free {
                    // Allocate all these frames
                    for j in 0..count_u64 {
                        let idx = start_idx + offset + j;
                        let word = (idx as usize) / 64;
                        let bit = (idx as usize) % 64;
                        self.bitmap[word] &= !(1u64 << bit);
                    }
                    self.allocated_frames += count_u64;
                    return Some(r.start + (offset * FRAME_SIZE));
                }
            }
        }
        None
    }

    fn get_region_start_index(&self, region_idx: usize) -> u64 {
        let mut idx = 0u64;
        for i in 0..region_idx {
            idx += self.regions[i].frame_count;
        }
        idx
    }

    /// Deallocate a physical frame.
    ///
    /// Returns true if the frame was successfully released, false if it was
    /// not allocated, was reserved, or is invalid (double-release rejected).
    pub fn deallocate_frame(&mut self, addr: u64) -> bool {
        if addr % FRAME_SIZE != 0 {
            return false;
        }
        if !self.is_valid_frame(addr) {
            return false;
        }
        // Check if reserved
        if self.is_reserved(addr) {
            return false;
        }
        let frame_idx = self.addr_to_index(addr);
        if frame_idx >= self.total_frames {
            return false;
        }
        let word = (frame_idx as usize) / 64;
        let bit = (frame_idx as usize) % 64;
        if word >= self.bitmap_words {
            return false;
        }
        // Check if already free (double release)
        if (self.bitmap[word] & (1u64 << bit)) != 0 {
            return false;
        }
        // Free the frame
        self.bitmap[word] |= 1u64 << bit;
        if self.allocated_frames > 0 {
            self.allocated_frames -= 1;
        }
        true
    }

    /// Deallocate a run of consecutive physical frames.
    pub fn deallocate_contiguous_frames(&mut self, addr: u64, count: usize) -> bool {
        if count == 0 {
            return true;
        }
        let mut all_valid = true;
        for i in 0..count {
            let frame_addr = addr + (i as u64) * FRAME_SIZE;
            if !self.is_valid_frame(frame_addr) || self.is_reserved(frame_addr) {
                all_valid = false;
                break;
            }
            let frame_idx = self.addr_to_index(frame_addr);
            if frame_idx >= self.total_frames {
                all_valid = false;
                break;
            }
            let word = (frame_idx as usize) / 64;
            let bit = (frame_idx as usize) % 64;
            if word >= self.bitmap_words || (self.bitmap[word] & (1u64 << bit)) != 0 {
                // Double release or not allocated
                all_valid = false;
                break;
            }
        }
        if !all_valid {
            return false;
        }
        // Release all frames
        for i in 0..count {
            let frame_addr = addr + (i as u64) * FRAME_SIZE;
            let frame_idx = self.addr_to_index(frame_addr);
            let word = (frame_idx as usize) / 64;
            let bit = (frame_idx as usize) % 64;
            self.bitmap[word] |= 1u64 << bit;
            if self.allocated_frames > 0 {
                self.allocated_frames -= 1;
            }
        }
        true
    }
}

impl Default for PhysFrameAllocator {
    fn default() -> Self {
        Self::new()
    }
}
