//! The Elvaris kernel heap.
//!
//! The heap is the kernel's general-purpose allocator. It owns a reserved
//! virtual-address range, maps the beginning of that range through the
//! virtual-memory subsystem, and hands the mapped region to the pure
//! [`allocator::Arena`] that does the actual allocation work.
//!
//! # Layering
//!
//! ```text
//! Heap allocator (allocator::Arena):
//!     chooses virtual addresses inside the heap
//!
//! Virtual-memory subsystem (vmm::mapper):
//!     maps virtual pages
//!
//! PMM (pmm::phys_alloc):
//!     supplies physical frames
//!
//! Bootloader:
//!     no longer manages ordinary heap allocations
//! ```
//!
//! The heap never touches a page table and never computes a physical address
//! itself: it asks the mapper for pages, and the mapper asks the PMM for
//! frames. Failures on either side are propagated unchanged, never swallowed.
//!
//! # Address range
//!
//! The heap lives at [`HEAP_VIRT_BASE`], in its own PML4 entry, and reserves
//! [`HEAP_RESERVED_LEN`] bytes of virtual address space. Only
//! [`HEAP_MAPPED_PAGES`] pages are mapped during initialization; growing the
//! heap on demand is not implemented in this milestone.
//!
//! The range is provisional: the final Elvaris address-space design does not
//! exist yet. See `docs/milestone-5.md`.
//!
//! # Limitations
//!
//! - The heap is a fixed region. It is not grown on demand.
//! - The PMM cannot reclaim frames, so unmapped heap pages would not return
//!   their physical memory. Because the heap is not grown, this does not
//!   happen in this milestone.
//! - The allocator itself is single-threaded. The global API serialises
//!   access with a minimal lock, which is not a full SMP synchronization
//!   design.

#![allow(dead_code)]

pub mod allocator;

#[cfg(test)]
mod tests;

use core::cell::UnsafeCell;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::serial;
use crate::vmm::mapper::{MapError, Mapper};
use crate::vmm::page_table::{entry_flags, pml4_index, PAGE_SIZE};

use allocator::{AllocError, Arena, ArenaStats};

/// Provisional base address of the kernel heap (PML4 entry 320).
pub const HEAP_VIRT_BASE: u64 = 0xFFFF_A000_0000_0000;

/// Provisional size of the reserved heap address range (64 MiB).
pub const HEAP_RESERVED_LEN: u64 = 0x0400_0000;

/// Number of heap pages mapped during initialization (1 MiB).
pub const HEAP_MAPPED_PAGES: usize = 256;

/// Size of the initially mapped heap region in bytes.
pub const HEAP_MAPPED_LEN: u64 = HEAP_MAPPED_PAGES as u64 * PAGE_SIZE;

/// Page-table flags used for heap pages: present, writable, not executable.
const HEAP_PAGE_FLAGS: u64 =
    entry_flags::PRESENT | entry_flags::WRITABLE | entry_flags::NO_EXECUTE;

/// Errors reported by the kernel heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeapError {
    /// The heap has not been initialized yet.
    NotInitialized,
    /// The heap has already been initialized.
    AlreadyInitialized,
    /// The heap has no free block that satisfies the request.
    OutOfMemory,
    /// The requested alignment is invalid or cannot be satisfied.
    InvalidAlignment,
    /// The request size overflows.
    SizeOverflow,
    /// The pointer does not belong to the heap.
    InvalidPointer,
    /// The block was already freed.
    DoubleFree,
    /// The size passed to `dealloc` does not match the block.
    SizeMismatch,
    /// The heap's internal state is inconsistent.
    Corrupted,
    /// The heap range overlaps an existing mapping.
    Overlap,
    /// The virtual-memory subsystem refused a mapping.
    Mapping(MapError),
}

impl From<AllocError> for HeapError {
    fn from(err: AllocError) -> Self {
        match err {
            AllocError::OutOfMemory => HeapError::OutOfMemory,
            AllocError::InvalidAlignment => HeapError::InvalidAlignment,
            AllocError::SizeOverflow => HeapError::SizeOverflow,
            AllocError::InvalidPointer => HeapError::InvalidPointer,
            AllocError::DoubleFree => HeapError::DoubleFree,
            AllocError::SizeMismatch => HeapError::SizeMismatch,
            AllocError::Corrupted => HeapError::Corrupted,
        }
    }
}

impl From<MapError> for HeapError {
    fn from(err: MapError) -> Self {
        HeapError::Mapping(err)
    }
}

/// Statistics describing the state of the kernel heap.
#[derive(Debug, Clone, Copy)]
pub struct HeapStats {
    /// Base address of the heap.
    pub base: u64,
    /// Size of the reserved (but not necessarily mapped) heap range.
    pub reserved_len: u64,
    /// Number of pages mapped for the heap.
    pub mapped_pages: usize,
    /// Physical address of the first heap frame.
    pub first_frame: u64,
    /// Physical address of the last heap frame.
    pub last_frame: u64,
    /// Allocator accounting.
    pub arena: ArenaStats,
}

/// The kernel heap: a mapped region plus the allocator that manages it.
pub struct KernelHeap {
    arena: Arena,
    mapped_pages: usize,
    first_frame: u64,
    last_frame: u64,
}

impl KernelHeap {
    /// Allocate `size` bytes with the given alignment.
    ///
    /// The returned pointer always lies inside the reserved heap range.
    pub fn alloc(&mut self, size: usize, align: usize) -> Result<NonNull<u8>, HeapError> {
        if size == 0 && align as u64 > HEAP_RESERVED_LEN {
            // A zero-sized allocation is placed at the end of the mapped
            // region, so a stricter alignment would leave the heap range.
            return Err(HeapError::InvalidAlignment);
        }
        let ptr = self.arena.alloc(size, align).map_err(HeapError::from)?;
        let address = ptr.as_ptr() as u64;
        if address < HEAP_VIRT_BASE || address >= HEAP_VIRT_BASE + HEAP_RESERVED_LEN {
            return Err(HeapError::InvalidPointer);
        }
        Ok(ptr)
    }

    /// Return a previously allocated block to the heap.
    pub fn dealloc(&mut self, ptr: NonNull<u8>, size: usize) -> Result<(), HeapError> {
        self.arena.dealloc(ptr, size).map_err(HeapError::from)
    }

    /// Current statistics.
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            base: self.arena.base(),
            reserved_len: HEAP_RESERVED_LEN,
            mapped_pages: self.mapped_pages,
            first_frame: self.first_frame,
            last_frame: self.last_frame,
            arena: self.arena.stats(),
        }
    }
}

/// A minimal spin lock.
///
/// The kernel heap is the only user. The lock is deliberately simple: it is
/// not fair, has no priority inheritance, and is not safe to take in an
/// interrupt context that can interrupt a thread that already holds it. A
/// full SMP synchronization design is deferred to a later milestone.
pub struct SpinLock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

// SAFETY: access to the data is serialized through the lock.
unsafe impl<T: Send> Sync for SpinLock<T> {}
// SAFETY: the lock can be moved between threads.
unsafe impl<T: Send> Send for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(value),
        }
    }

    /// Acquire the lock, spinning until it is available.
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        SpinLockGuard { lock: self }
    }
}

/// Guard returned by [`SpinLock::lock`]; releases the lock when dropped.
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> core::ops::Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: the guard holds the lock, so no other thread can mutate the
        // data concurrently.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> core::ops::DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: the guard holds the lock, so no other thread can mutate the
        // data concurrently.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

/// The single kernel heap instance.
static HEAP: SpinLock<Option<KernelHeap>> = SpinLock::new(None);

/// Initialize the kernel heap.
///
/// The initial heap pages are mapped through the virtual-memory subsystem,
/// which obtains their physical frames from the PMM. If any step fails, the
/// error is returned unchanged: the heap does not hide PMM or mapping
/// failures.
pub fn init(mapper: &mut Mapper) -> Result<(), HeapError> {
    // The heap range must not overlap an existing mapping. Because the heap
    // occupies a PML4 entry of its own, an unused entry proves that no other
    // mapping shares the range.
    let entry = pml4_index(HEAP_VIRT_BASE);
    if mapper.pml4_is_used(entry) {
        return Err(HeapError::Overlap);
    }

    // Map the initial heap pages. The mapper takes the physical frames from
    // the PMM and reports any failure.
    mapper.map_range(HEAP_VIRT_BASE, HEAP_MAPPED_PAGES, HEAP_PAGE_FLAGS)?;

    // Read the mapping back: the pages must be mapped before they are used.
    let first_frame = mapper
        .translate(HEAP_VIRT_BASE)
        .ok_or(HeapError::Mapping(MapError::NotMapped))?
        .0;
    let last_frame = mapper
        .translate(HEAP_VIRT_BASE + HEAP_MAPPED_LEN - PAGE_SIZE)
        .ok_or(HeapError::Mapping(MapError::NotMapped))?
        .0;

    // Zero the region: the allocator only initialises its own metadata, and
    // the kernel must not hand out uninitialised memory.
    // SAFETY: the range was just mapped and is writable.
    unsafe {
        core::ptr::write_bytes(HEAP_VIRT_BASE as *mut u8, 0, HEAP_MAPPED_LEN as usize);
    }

    // Hand the mapped region to the allocator.
    let arena = Arena::new(HEAP_VIRT_BASE as usize, HEAP_MAPPED_LEN as usize)
        .ok_or(HeapError::Corrupted)?;

    let heap = KernelHeap {
        arena,
        mapped_pages: HEAP_MAPPED_PAGES,
        first_frame,
        last_frame,
    };

    let mut guard = HEAP.lock();
    if guard.is_some() {
        return Err(HeapError::AlreadyInitialized);
    }
    *guard = Some(heap);
    Ok(())
}

/// Whether the heap has been initialized.
pub fn is_initialized() -> bool {
    HEAP.lock().is_some()
}

/// Allocate `size` bytes with the given alignment from the kernel heap.
///
/// The pointer is aligned to `align`, which must be a power of two.
pub fn alloc(size: usize, align: usize) -> Result<NonNull<u8>, HeapError> {
    let mut guard = HEAP.lock();
    let heap = guard.as_mut().ok_or(HeapError::NotInitialized)?;
    heap.alloc(size, align)
}

/// Return a block to the kernel heap.
///
/// `size` must be the size the block was allocated with.
pub fn dealloc(ptr: NonNull<u8>, size: usize) -> Result<(), HeapError> {
    let mut guard = HEAP.lock();
    let heap = guard.as_mut().ok_or(HeapError::NotInitialized)?;
    heap.dealloc(ptr, size)
}

/// Current heap statistics.
pub fn stats() -> Result<HeapStats, HeapError> {
    let guard = HEAP.lock();
    let heap = guard.as_ref().ok_or(HeapError::NotInitialized)?;
    Ok(heap.stats())
}

/// Check the internal consistency of the heap.
pub fn validate() -> Result<(), HeapError> {
    let guard = HEAP.lock();
    let heap = guard.as_ref().ok_or(HeapError::NotInitialized)?;
    heap.arena.validate().map_err(HeapError::from)
}

/// Run a boot-time self test of the heap.
///
/// The test allocates, writes, verifies, frees and exhausts the heap. It
/// leaves the heap in the state it had before the test: every block that the
/// test allocated is released again.
pub fn self_test() -> Result<(), HeapError> {
    // 1. A normal allocation: write a pattern and read it back.
    let block = alloc(64, 16)?;
    // SAFETY: the block is a live allocation of 64 bytes.
    unsafe {
        let bytes = block.as_ptr() as *mut u8;
        for index in 0..64usize {
            bytes.add(index).write(index as u8);
        }
        for index in 0..64usize {
            if bytes.add(index).read() != index as u8 {
                return Err(HeapError::Corrupted);
            }
        }
    }
    dealloc(block, 64)?;
    unsafe {
        serial::write_str("Elvaris kernel: heap: self_test: alloc ok\r\n");
    }

    // 2. A page-aligned allocation.
    let page = alloc(PAGE_SIZE as usize, PAGE_SIZE as usize)?;
    if (page.as_ptr() as u64) % PAGE_SIZE != 0 {
        return Err(HeapError::InvalidAlignment);
    }
    dealloc(page, PAGE_SIZE as usize)?;
    unsafe {
        serial::write_str("Elvaris kernel: heap: self_test: free ok\r\n");
    }

    // 3. Exhaustion: allocate until the heap refuses, then release
    //    everything again.
    const CHUNK: usize = 64 * 1024;
    const MAX_CHUNKS: usize = 32;
    let mut chunks: [Option<NonNull<u8>>; MAX_CHUNKS] = [None; MAX_CHUNKS];
    let mut count = 0usize;
    loop {
        match alloc(CHUNK, 16) {
            Ok(ptr) => {
                if count == MAX_CHUNKS {
                    break;
                }
                chunks[count] = Some(ptr);
                count += 1;
            }
            Err(HeapError::OutOfMemory) => break,
            Err(err) => return Err(err),
        }
    }
    if count == MAX_CHUNKS {
        // The heap should have been exhausted before this many chunks.
        return Err(HeapError::Corrupted);
    }
    for chunk in chunks.iter().flatten() {
        dealloc(*chunk, CHUNK)?;
    }
    unsafe {
        serial::write_str("Elvaris kernel: heap: self_test: exhaustion ok\r\n");
    }

    // 4. The heap must be consistent and empty again.
    validate()?;
    let stats = stats()?;
    if stats.arena.used_bytes != 0 {
        return Err(HeapError::Corrupted);
    }
    Ok(())
}

/// Print the heap configuration and statistics to the serial console.
pub fn report() {
    let stats = match stats() {
        Ok(stats) => stats,
        Err(err) => {
            unsafe {
                serial::write_str("Elvaris kernel: heap: unavailable: ");
                serial::write_fmt(core::format_args!("{:?}", err));
                serial::write_newline();
            }
            return;
        }
    };

    unsafe {
        serial::write_str("Elvaris kernel: heap: base=0x");
        serial::write_bytes(serial::format_hex64(
            &mut [0u8; 64],
            b"",
            stats.base,
            16,
        ));
        serial::write_str(" reserved_bytes=");
        serial::write_fmt(core::format_args!("{}", stats.reserved_len));
        serial::write_str(" mapped_pages=");
        serial::write_fmt(core::format_args!("{}", stats.mapped_pages));
        serial::write_newline();

        serial::write_str("Elvaris kernel: heap: frames=0x");
        serial::write_bytes(serial::format_hex64(
            &mut [0u8; 64],
            b"",
            stats.first_frame,
            16,
        ));
        serial::write_str("..0x");
        serial::write_bytes(serial::format_hex64(
            &mut [0u8; 64],
            b"",
            stats.last_frame,
            16,
        ));
        serial::write_newline();

        serial::write_str("Elvaris kernel: heap: capacity=");
        serial::write_fmt(core::format_args!("{}", stats.arena.capacity));
        serial::write_str(" used=");
        serial::write_fmt(core::format_args!("{}", stats.arena.used_bytes));
        serial::write_str(" free=");
        serial::write_fmt(core::format_args!("{}", stats.arena.free_bytes));
        serial::write_str(" blocks=");
        serial::write_fmt(core::format_args!("{}", stats.arena.block_count));
        serial::write_newline();
    }
}
