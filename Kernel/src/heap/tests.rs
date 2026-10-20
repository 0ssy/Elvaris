//! Unit tests for the pure heap allocator.
//!
//! These tests run on the host target. They exercise the allocator logic
//! only: no paging, no PMM and no kernel environment is involved.

use alloc::vec::Vec;

use super::allocator::{AllocError, Arena, HEADER_ALIGN, HEADER_SIZE, MIN_PAYLOAD};

/// Size of the test arenas. A multiple of the page size keeps the arena
/// page-aligned, which is what the kernel heap uses.
const ARENA_SIZE: usize = 64 * 1024;

/// Create an arena over a freshly allocated, 16-byte aligned buffer.
///
/// The buffer is intentionally leaked: the arena stores raw pointers into it
/// and the tests outlive individual buffers.
fn make_arena(size: usize) -> Arena {
    let mut words = vec![0u128; size / 16];
    let base = words.as_mut_ptr() as usize;
    std::mem::forget(words);
    Arena::new(base, size).expect("arena should be created")
}

#[test]
fn arena_rejects_invalid_regions() {
    // Unaligned base address.
    assert!(Arena::new(1, ARENA_SIZE).is_none());
    // Size that is not a multiple of the header alignment.
    assert!(Arena::new(0, ARENA_SIZE + 1).is_none());
    // Region too small to hold a block.
    assert!(Arena::new(0, HEADER_SIZE).is_none());
    assert!(Arena::new(0, 0).is_none());
}

#[test]
fn arena_starts_empty() {
    let arena = make_arena(ARENA_SIZE);
    let stats = arena.stats();
    assert_eq!(stats.capacity, ARENA_SIZE as u64);
    assert_eq!(stats.used_bytes, 0);
    assert_eq!(stats.free_bytes, (ARENA_SIZE - HEADER_SIZE) as u64);
    assert_eq!(stats.block_count, 1);
    assert_eq!(stats.allocations, 0);
    assert_eq!(stats.frees, 0);
    assert!(arena.validate().is_ok());
}

#[test]
fn basic_allocation() {
    let mut arena = make_arena(ARENA_SIZE);
    let ptr = arena.alloc(64, 16).expect("allocation should succeed");

    let address = ptr.as_ptr() as usize;
    assert!(arena.contains(address as u64));
    assert_eq!(address % HEADER_ALIGN, 0);

    let stats = arena.stats();
    assert_eq!(stats.allocations, 1);
    assert!(stats.used_bytes >= 64);
    assert!(stats.used_bytes < stats.capacity);
    assert!(arena.validate().is_ok());
}

#[test]
fn multiple_allocations_do_not_overlap() {
    let mut arena = make_arena(ARENA_SIZE);
    let mut blocks = [core::ptr::null_mut::<u8>(); 16];
    let mut ranges: Vec<(usize, usize)> = Vec::new();

    for index in 0..blocks.len() {
        let size = 32 + index * 7;
        let ptr = arena.alloc(size, 16).expect("allocation should succeed");
        blocks[index] = ptr.as_ptr();
        ranges.push((ptr.as_ptr() as usize, ptr.as_ptr() as usize + size));
    }

    // No two live allocations may overlap.
    for first in 0..ranges.len() {
        for second in (first + 1)..ranges.len() {
            let (a_start, a_end) = ranges[first];
            let (b_start, b_end) = ranges[second];
            assert!(
                a_end <= b_start || b_end <= a_start,
                "allocations {first} and {second} overlap"
            );
        }
    }

    for block in blocks.iter() {
        arena.dealloc(core::ptr::NonNull::new(*block).unwrap(), 32)
            .ok();
    }
    assert!(arena.validate().is_ok());
}

#[test]
fn alignment_is_respected() {
    let mut arena = make_arena(ARENA_SIZE);
    for align in [1usize, 2, 4, 8, 16, 32, 64, 128, 4096] {
        let ptr = arena.alloc(100, align).expect("allocation should succeed");
        let address = ptr.as_ptr() as usize;
        assert_eq!(address % align, 0, "alignment {align} not honoured");
        assert!(arena.contains(address as u64));
        arena.dealloc(ptr, 100).expect("deallocation should succeed");
    }
    assert!(arena.validate().is_ok());
}

#[test]
fn deallocation_returns_memory() {
    let mut arena = make_arena(ARENA_SIZE);
    let before = arena.stats();

    let ptr = arena.alloc(128, 16).expect("allocation should succeed");
    let during = arena.stats();
    assert!(during.used_bytes > 0);

    arena.dealloc(ptr, 128).expect("deallocation should succeed");
    let after = arena.stats();
    assert_eq!(after.used_bytes, 0);
    assert_eq!(after.free_bytes, before.free_bytes);
    assert_eq!(after.frees, 1);
    assert!(arena.validate().is_ok());
}

#[test]
fn freed_block_is_reused() {
    let mut arena = make_arena(ARENA_SIZE);
    let first = arena.alloc(64, 16).expect("allocation should succeed");
    arena.dealloc(first, 64).expect("deallocation should succeed");
    let second = arena.alloc(64, 16).expect("allocation should succeed");
    assert_eq!(first, second, "freed block should be reused");
    assert!(arena.validate().is_ok());
}

#[test]
fn adjacent_free_blocks_are_coalesced() {
    let mut arena = make_arena(ARENA_SIZE);
    let a = arena.alloc(64, 16).expect("allocation should succeed");
    let b = arena.alloc(64, 16).expect("allocation should succeed");
    let c = arena.alloc(64, 16).expect("allocation should succeed");

    let after_allocs = arena.stats();
    assert_eq!(after_allocs.block_count, 4);

    // Freeing the middle block cannot merge anything yet.
    arena.dealloc(b, 64).expect("deallocation should succeed");
    assert_eq!(arena.stats().block_count, 4);

    // Freeing `a` merges it with `b`.
    arena.dealloc(a, 64).expect("deallocation should succeed");
    assert_eq!(arena.stats().block_count, 3);

    // Freeing `c` merges it with the merged block and the trailing remainder.
    arena.dealloc(c, 64).expect("deallocation should succeed");
    let stats = arena.stats();
    assert_eq!(stats.block_count, 1);
    assert_eq!(stats.used_bytes, 0);
    assert_eq!(stats.free_bytes, (ARENA_SIZE - HEADER_SIZE) as u64);
    assert!(arena.validate().is_ok());
}

#[test]
fn fragmentation_does_not_corrupt_the_heap() {
    let mut arena = make_arena(ARENA_SIZE);
    let mut blocks: Vec<(core::ptr::NonNull<u8>, usize)> = Vec::new();

    for index in 0..64usize {
        let size = 24 + (index % 7) * 16;
        let ptr = arena.alloc(size, 16).expect("allocation should succeed");
        blocks.push((ptr, size));
    }
    assert!(arena.validate().is_ok());

    // Free every second block, then the rest.
    for (index, (ptr, size)) in blocks.iter().enumerate() {
        if index % 2 == 0 {
            arena
                .dealloc(*ptr, *size)
                .expect("deallocation should succeed");
        }
    }
    assert!(arena.validate().is_ok());

    for (index, (ptr, size)) in blocks.iter().enumerate() {
        if index % 2 == 1 {
            arena
                .dealloc(*ptr, *size)
                .expect("deallocation should succeed");
        }
    }

    let stats = arena.stats();
    assert_eq!(stats.used_bytes, 0);
    assert_eq!(stats.block_count, 1);
    assert!(arena.validate().is_ok());
}

#[test]
fn zero_sized_allocation_is_defined() {
    let mut arena = make_arena(ARENA_SIZE);
    let ptr = arena.alloc(0, 16).expect("zero-sized allocation should succeed");
    assert!(!ptr.as_ptr().is_null());
    assert_eq!(ptr.as_ptr() as usize % 16, 0);

    // A zero-sized allocation owns no memory: it is placed at the end of the
    // arena, outside every block, so it cannot alias the allocator metadata.
    assert!(!arena.contains(ptr.as_ptr() as u64));

    // The heap is unchanged by a zero-sized allocation.
    let stats = arena.stats();
    assert_eq!(stats.used_bytes, 0);
    assert_eq!(stats.allocations, 1);

    // Releasing a zero-sized allocation is a no-op.
    arena.dealloc(ptr, 0).expect("deallocation should succeed");
    assert_eq!(arena.stats().used_bytes, 0);
    assert!(arena.validate().is_ok());
}

#[test]
fn exhaustion_is_reported() {
    let mut arena = make_arena(ARENA_SIZE);

    // A request larger than the arena can never be satisfied.
    let err = arena.alloc(ARENA_SIZE * 2, 16).unwrap_err();
    assert_eq!(err, AllocError::OutOfMemory);

    // Fill the heap with large blocks until it refuses.
    let mut count = 0usize;
    loop {
        match arena.alloc(ARENA_SIZE / 4, 16) {
            Ok(_) => {
                count += 1;
                assert!(count < 64, "heap should have been exhausted");
            }
            Err(AllocError::OutOfMemory) => break,
            Err(err) => panic!("unexpected error: {err:?}"),
        }
    }
    assert!(count > 0);
    assert!(arena.validate().is_ok());
}

#[test]
fn invalid_requests_are_rejected() {
    let mut arena = make_arena(ARENA_SIZE);
    assert_eq!(arena.alloc(16, 0).unwrap_err(), AllocError::InvalidAlignment);
    assert_eq!(arena.alloc(16, 3).unwrap_err(), AllocError::InvalidAlignment);
    assert_eq!(arena.alloc(16, 24).unwrap_err(), AllocError::InvalidAlignment);
    assert_eq!(
        arena.alloc(usize::MAX, 16).unwrap_err(),
        AllocError::SizeOverflow
    );
    assert!(arena.validate().is_ok());
}

#[test]
fn double_free_is_rejected() {
    let mut arena = make_arena(ARENA_SIZE);
    let ptr = arena.alloc(64, 16).expect("allocation should succeed");
    arena.dealloc(ptr, 64).expect("deallocation should succeed");
    assert_eq!(
        arena.dealloc(ptr, 64).unwrap_err(),
        AllocError::DoubleFree
    );
    assert!(arena.validate().is_ok());
}

#[test]
fn foreign_pointers_are_rejected() {
    let mut arena = make_arena(ARENA_SIZE);
    let mut other = make_arena(ARENA_SIZE);
    let foreign = other.alloc(64, 16).expect("allocation should succeed");

    assert_eq!(
        arena.dealloc(foreign, 64).unwrap_err(),
        AllocError::InvalidPointer
    );

    // A pointer that is not inside the arena at all.
    let outside = core::ptr::NonNull::new(0x1000 as *mut u8).unwrap();
    assert_eq!(
        arena.dealloc(outside, 64).unwrap_err(),
        AllocError::InvalidPointer
    );
    assert!(arena.validate().is_ok());
}

#[test]
fn size_mismatch_is_rejected() {
    let mut arena = make_arena(ARENA_SIZE);
    let ptr = arena.alloc(64, 16).expect("allocation should succeed");
    assert_eq!(
        arena.dealloc(ptr, 4096).unwrap_err(),
        AllocError::SizeMismatch
    );
    assert!(arena.validate().is_ok());
}

#[test]
fn allocation_never_leaves_the_arena() {
    let mut arena = make_arena(ARENA_SIZE);
    for _ in 0..32 {
        let ptr = arena.alloc(17, 4096).expect("allocation should succeed");
        let address = ptr.as_ptr() as usize;
        assert!(arena.contains(address as u64));
        assert_eq!(address % 4096, 0);
        arena.dealloc(ptr, 17).expect("deallocation should succeed");
    }
    assert!(arena.validate().is_ok());
}

#[test]
fn small_payloads_are_served() {
    let mut arena = make_arena(ARENA_SIZE);
    let ptr = arena.alloc(1, 1).expect("allocation should succeed");
    assert!(arena.contains(ptr.as_ptr() as u64));
    arena.dealloc(ptr, 1).expect("deallocation should succeed");
    assert!(arena.validate().is_ok());
    assert!(MIN_PAYLOAD <= HEADER_SIZE);
}
