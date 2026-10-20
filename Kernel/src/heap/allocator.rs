//! A linked-list heap allocator over a caller-supplied memory region.
//!
//! This module contains the *pure* part of the kernel heap: it knows how to
//! split, allocate, free and coalesce blocks inside a contiguous region of
//! memory, and nothing else. It does not know about page tables, physical
//! frames or the PMM; the kernel heap front-end (`super::KernelHeap`) obtains
//! the region from the virtual-memory subsystem and hands it to the arena.
//!
//! Keeping the logic free of paging concerns means it can be unit tested on
//! the host target without a kernel environment.
//!
//! # Block layout
//!
//! ```text
//! ┌──────────────┬───────────────────────────────┐
//! │ block header │ payload                       │
//! │ 32 bytes     │ capacity bytes                │
//! └──────────────┴───────────────────────────────┘
//! ```
//!
//! Blocks form a physically ordered, doubly linked list. The header of a block
//! always precedes its payload, so a free block can be merged with its
//! physical neighbours in both directions.
//!
//! # Alignment
//!
//! The header is 32 bytes long and 16-byte aligned, so every payload starts on
//! a 16-byte boundary. Allocations that require a stricter alignment are
//! over-aligned inside their block; a back-pointer is stored immediately in
//! front of the returned address so that the block can always be recovered.

#![allow(dead_code)]

use core::ptr::NonNull;

/// Size of a block header in bytes.
pub const HEADER_SIZE: usize = 32;

/// Alignment guaranteed for every block header and every block payload.
pub const HEADER_ALIGN: usize = 16;

/// Smallest payload a free block may keep after a split.
pub const MIN_PAYLOAD: usize = 16;

/// Marker stored in front of a normally aligned user pointer.
const ALIGN_TAG: usize = 0x454C_5641_5249_5300; // "ELVARIS\0"

/// Bit 0 of the stored size marks a block as in use.
const IN_USE: usize = 1;

/// Errors reported by the arena.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AllocError {
    /// No free block is large enough for the request.
    OutOfMemory,
    /// The requested alignment is not a power of two, or cannot be satisfied
    /// inside the arena.
    InvalidAlignment,
    /// The request size overflows when rounded up to the allocator's
    /// granularity.
    SizeOverflow,
    /// The pointer does not belong to this arena, or does not point at the
    /// start of a block payload.
    InvalidPointer,
    /// The block was already free.
    DoubleFree,
    /// The size passed to `dealloc` is larger than the block can hold.
    SizeMismatch,
    /// The arena's internal state is inconsistent.
    Corrupted,
}

/// Header stored in front of every block.
#[repr(C, align(16))]
struct BlockHeader {
    /// Payload capacity in bytes. Bit 0 is the in-use flag; the remaining bits
    /// hold the capacity and are always a multiple of [`HEADER_ALIGN`].
    size: usize,
    /// Next block in address order.
    next: *mut BlockHeader,
    /// Previous block in address order.
    prev: *mut BlockHeader,
    /// The word that precedes the user pointer of a normally aligned
    /// allocation: [`ALIGN_TAG`]. For over-aligned allocations the block
    /// address is stored in front of the user pointer instead.
    back: usize,
}

impl BlockHeader {
    fn capacity(&self) -> usize {
        self.size & !IN_USE
    }

    fn is_free(&self) -> bool {
        self.size & IN_USE == 0
    }

    fn set_in_use(&mut self) {
        self.size |= IN_USE;
    }

    fn set_free(&mut self) {
        self.size &= !IN_USE;
    }

    fn payload(&self) -> usize {
        self as *const BlockHeader as usize + HEADER_SIZE
    }
}

/// Snapshot of the arena's accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArenaStats {
    /// Base address of the arena.
    pub base: u64,
    /// Total size of the arena in bytes.
    pub capacity: u64,
    /// Payload bytes currently handed out.
    pub used_bytes: u64,
    /// Payload bytes currently free.
    pub free_bytes: u64,
    /// Number of blocks in the arena (free and in use).
    pub block_count: u64,
    /// Number of successful allocations since the arena was created.
    pub allocations: u64,
    /// Number of successful deallocations since the arena was created.
    pub frees: u64,
}

/// A linked-list allocator over a contiguous region of memory.
///
/// # Safety invariants
///
/// The arena never reads or writes outside `[base, base + size)`, except for
/// the eight bytes in front of a user pointer, which are part of the block's
/// own payload area.
pub struct Arena {
    base: usize,
    size: usize,
    head: *mut BlockHeader,
    used_bytes: usize,
    free_bytes: usize,
    block_count: usize,
    allocations: usize,
    frees: usize,
}

// SAFETY: the raw pointers in the arena refer to the memory region the arena
// was created over. That region is owned by the arena's owner, and the arena
// is only ever accessed while the owner's lock is held, so moving the arena
// between threads does not introduce a data race.
unsafe impl Send for Arena {}

impl Arena {
    /// Create an arena over the region `[base, base + size)`.
    ///
    /// The region must be writable, `base` must be [`HEADER_ALIGN`]-aligned and
    /// `size` must be a multiple of [`HEADER_ALIGN`]. The first block header
    /// is written into the region.
    ///
    /// Returns `None` if the region is too small to hold a block.
    pub fn new(base: usize, size: usize) -> Option<Arena> {
        if base % HEADER_ALIGN != 0 {
            return None;
        }
        if size % HEADER_ALIGN != 0 {
            return None;
        }
        if size < HEADER_SIZE + MIN_PAYLOAD {
            return None;
        }
        if base.checked_add(size).is_none() {
            return None;
        }

        let head = base as *mut BlockHeader;
        unsafe {
            (*head).size = (size - HEADER_SIZE) & !IN_USE;
            (*head).next = core::ptr::null_mut();
            (*head).prev = core::ptr::null_mut();
            (*head).back = ALIGN_TAG;
        }

        Some(Arena {
            base,
            size,
            head,
            used_bytes: 0,
            free_bytes: size - HEADER_SIZE,
            block_count: 1,
            allocations: 0,
            frees: 0,
        })
    }

    /// Base address of the arena.
    pub fn base(&self) -> u64 {
        self.base as u64
    }

    /// Size of the arena in bytes.
    pub fn size(&self) -> u64 {
        self.size as u64
    }

    /// Whether `addr` lies inside the arena.
    pub fn contains(&self, addr: u64) -> bool {
        let addr = addr as usize;
        addr >= self.base && addr < self.base + self.size
    }

    /// Current accounting snapshot.
    pub fn stats(&self) -> ArenaStats {
        ArenaStats {
            base: self.base as u64,
            capacity: self.size as u64,
            used_bytes: self.used_bytes as u64,
            free_bytes: self.free_bytes as u64,
            block_count: self.block_count as u64,
            allocations: self.allocations as u64,
            frees: self.frees as u64,
        }
    }

    /// Allocate `size` bytes with the given alignment.
    ///
    /// A zero-sized allocation succeeds and returns a non-null, aligned
    /// pointer that owns no memory. The pointer must not be dereferenced and
    /// must only be passed back to [`Arena::dealloc`] with a size of zero.
    pub fn alloc(&mut self, size: usize, align: usize) -> Result<NonNull<u8>, AllocError> {
        if align == 0 || !align.is_power_of_two() {
            return Err(AllocError::InvalidAlignment);
        }
        let align = if align < HEADER_ALIGN { HEADER_ALIGN } else { align };

        if size == 0 {
            return self.zero_sized(align);
        }
        if size > usize::MAX - HEADER_ALIGN {
            return Err(AllocError::SizeOverflow);
        }

        // The worst case offset between the block payload and the aligned user
        // pointer is `align - HEADER_ALIGN`, because the payload itself is
        // always HEADER_ALIGN-aligned. The used part of the block is rounded up
        // to the allocator's granularity, so the requirement is rounded as well.
        let max_offset = align - HEADER_ALIGN;
        let needed = size
            .checked_add(max_offset)
            .ok_or(AllocError::SizeOverflow)?;
        let needed = needed
            .checked_add(HEADER_ALIGN - 1)
            .ok_or(AllocError::SizeOverflow)?
            & !(HEADER_ALIGN - 1);

        let mut current = self.head;
        while !current.is_null() {
            unsafe {
                if (*current).is_free() {
                    let capacity = (*current).capacity();
                    if capacity >= needed {
                        if let Some(result) = self.allocate_from(current, size, align) {
                            return Ok(result);
                        }
                    }
                }
                current = (*current).next;
            }
        }

        Err(AllocError::OutOfMemory)
    }

    /// Allocate from a known free block. Returns `None` if the block turns out
    /// to be too small for the aligned request.
    unsafe fn allocate_from(
        &mut self,
        block: *mut BlockHeader,
        size: usize,
        align: usize,
    ) -> Option<NonNull<u8>> {
        // SAFETY: `block` points at a live free block inside the arena.
        unsafe {
            let capacity = (*block).capacity();
            let payload = (*block).payload();
            let user = match payload.checked_add(align - 1) {
                Some(v) => v & !(align - 1),
                None => return None,
            };
            let offset = user - payload;

            // Round the used part up so that the following block stays aligned.
            let used = match offset.checked_add(size) {
                Some(v) => match v.checked_add(HEADER_ALIGN - 1) {
                    Some(v) => v & !(HEADER_ALIGN - 1),
                    None => return None,
                },
                None => return None,
            };
            if used > capacity {
                return None;
            }

            let remainder = capacity - used;
            let final_capacity = if remainder >= HEADER_SIZE + MIN_PAYLOAD {
                // Split: the remainder becomes a new free block.
                let new_block = (payload + used) as *mut BlockHeader;
                (*new_block).size = (remainder - HEADER_SIZE) & !IN_USE;
                (*new_block).next = (*block).next;
                (*new_block).prev = block;
                (*new_block).back = ALIGN_TAG;
                if !(*block).next.is_null() {
                    (*(*block).next).prev = new_block;
                }
                (*block).next = new_block;
                self.block_count += 1;
                self.free_bytes -= used + HEADER_SIZE;
                used
            } else {
                // Not enough room for another block: absorb the remainder.
                self.free_bytes -= capacity;
                capacity
            };

            (*block).size = final_capacity | IN_USE;
            self.used_bytes += final_capacity;
            self.allocations += 1;

            // Remember how the user pointer can be turned back into this block.
            if offset == 0 {
                (*block).back = ALIGN_TAG;
            } else {
                (*block).back = block as usize;
                // SAFETY: the word in front of the user pointer lies inside the
                // block's own payload area.
                *((user - 8) as *mut usize) = block as usize;
            }

            Some(NonNull::new(user as *mut u8).unwrap())
        }
    }

    /// Return a block to the free list.
    ///
    /// `size` is the size the block was allocated with; it is used to detect
    /// mismatched calls. Deallocating a zero-sized allocation is a no-op.
    pub fn dealloc(
        &mut self,
        ptr: NonNull<u8>,
        size: usize,
    ) -> Result<(), AllocError> {
        if size == 0 {
            // Zero-sized allocations own no memory.
            return Ok(());
        }

        let user = ptr.as_ptr() as usize;
        if user < self.base + HEADER_SIZE || user >= self.base + self.size {
            return Err(AllocError::InvalidPointer);
        }

        // SAFETY: the word in front of the user pointer is inside the arena.
        let tag = unsafe { *(user.wrapping_sub(8) as *const usize) };
        let header = if tag == ALIGN_TAG {
            (user - HEADER_SIZE) as *mut BlockHeader
        } else {
            tag as *mut BlockHeader
        };

        let header_addr = header as usize;
        if header_addr < self.base
            || header_addr % HEADER_ALIGN != 0
            || header_addr
                .checked_add(HEADER_SIZE)
                .map_or(true, |end| end > self.base + self.size)
        {
            return Err(AllocError::InvalidPointer);
        }

        // SAFETY: the header address was validated to lie inside the arena.
        let (payload, capacity) = unsafe { ((*header).payload(), (*header).capacity()) };
        if user < payload || user >= payload + capacity {
            return Err(AllocError::InvalidPointer);
        }
        if tag == ALIGN_TAG && user != payload {
            return Err(AllocError::InvalidPointer);
        }
        // SAFETY: the header is inside the arena and the block is in use.
        if unsafe { (*header).is_free() } {
            return Err(AllocError::DoubleFree);
        }
        if size > capacity {
            return Err(AllocError::SizeMismatch);
        }

        // SAFETY: the header is inside the arena and the block is in use.
        unsafe {
            (*header).set_free();
        }
        self.used_bytes -= capacity;
        self.free_bytes += capacity;
        self.frees += 1;

        self.coalesce(header);
        Ok(())
    }

    /// Merge `block` with its free physical neighbours.
    fn coalesce(&mut self, block: *mut BlockHeader) {
        // SAFETY: `block` is a live block inside the arena.
        unsafe {
            let next = (*block).next;
            if !next.is_null() && (*next).is_free() {
                let capacity = (*block).capacity() + HEADER_SIZE + (*next).capacity();
                (*block).size = capacity & !IN_USE;
                (*block).next = (*next).next;
                if !(*block).next.is_null() {
                    (*(*block).next).prev = block;
                }
                (*block).back = ALIGN_TAG;
                self.free_bytes += HEADER_SIZE;
                self.block_count -= 1;
            }

            let prev = (*block).prev;
            if !prev.is_null() && (*prev).is_free() {
                let capacity = (*prev).capacity() + HEADER_SIZE + (*block).capacity();
                (*prev).size = capacity & !IN_USE;
                (*prev).next = (*block).next;
                if !(*prev).next.is_null() {
                    (*(*prev).next).prev = prev;
                }
                (*prev).back = ALIGN_TAG;
                self.free_bytes += HEADER_SIZE;
                self.block_count -= 1;
            }
        }
    }

    /// Handle a zero-sized allocation request.
    ///
    /// The returned pointer is placed at the end of the arena: it is aligned,
    /// it owns no memory, and it is not part of any block, so writing through
    /// it cannot corrupt the allocator's metadata. It is released with
    /// [`Arena::dealloc`] and a size of zero, which is a no-op.
    fn zero_sized(&mut self, align: usize) -> Result<NonNull<u8>, AllocError> {
        let end = self.base + self.size;
        let user = match end.checked_add(align - 1) {
            Some(v) => v & !(align - 1),
            None => return Err(AllocError::InvalidAlignment),
        };
        self.allocations += 1;
        Ok(NonNull::new(user as *mut u8).unwrap())
    }

    /// Check the internal consistency of the arena.
    ///
    /// Used by the unit tests and by the boot-time self test.
    pub fn validate(&self) -> Result<(), AllocError> {
        let mut expected_addr = self.base;
        let mut previous = core::ptr::null_mut();
        let mut current = self.head;
        let mut free_total = 0usize;
        let mut used_total = 0usize;
        let mut blocks = 0usize;

        while !current.is_null() {
            let addr = current as usize;
            if addr != expected_addr {
                return Err(AllocError::Corrupted);
            }
            if addr % HEADER_ALIGN != 0 {
                return Err(AllocError::Corrupted);
            }
            // SAFETY: the block lies inside the arena.
            let (capacity, is_free) = unsafe { ((*current).capacity(), (*current).is_free()) };
            if capacity % HEADER_ALIGN != 0 {
                return Err(AllocError::Corrupted);
            }
            if addr + HEADER_SIZE + capacity > self.base + self.size {
                return Err(AllocError::Corrupted);
            }
            // SAFETY: the block lies inside the arena.
            if unsafe { (*current).prev } != previous {
                return Err(AllocError::Corrupted);
            }
            if is_free {
                free_total += capacity;
                // SAFETY: the block lies inside the arena.
                if unsafe { (*current).back } != ALIGN_TAG {
                    return Err(AllocError::Corrupted);
                }
            } else {
                used_total += capacity;
            }

            expected_addr = addr + HEADER_SIZE + capacity;
            previous = current;
            // SAFETY: the block lies inside the arena.
            current = unsafe { (*current).next };
            blocks += 1;
        }

        if expected_addr != self.base + self.size {
            return Err(AllocError::Corrupted);
        }
        if free_total != self.free_bytes || used_total != self.used_bytes {
            return Err(AllocError::Corrupted);
        }
        if blocks != self.block_count {
            return Err(AllocError::Corrupted);
        }
        Ok(())
    }
}
