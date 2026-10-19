//! Unit tests for physical memory management.

#![cfg(test)]

use super::frame::{FRAME_SIZE, Frame, normalize_region};
use super::phys_alloc::PhysFrameAllocator;
use bootloader_api::info::{MemoryRegion, MemoryRegionKind};

#[test]
fn frame_containing_address_aligned() {
    let addr = 0x1000;
    let f = Frame::containing_address(addr);
    assert_eq!(f.start_address(), 0x1000);
    assert_eq!(f.end_address(), 0x2000);
}

#[test]
fn frame_containing_address_unaligned() {
    let f = Frame::containing_address(0x1234);
    assert_eq!(f.start_address(), 0x1000);
    assert_eq!(f.end_address(), 0x2000);
}

#[test]
fn normalize_region_basic() {
    let r = MemoryRegion {
        kind: MemoryRegionKind::Usable,
        start: 0,
        end: 0x5000,
    };
    let norm = normalize_region(&r).unwrap();
    assert_eq!(norm.0, 0);
    assert_eq!(norm.1, 0x5000);
}

#[test]
fn normalize_region_unaligned() {
    let r = MemoryRegion {
        kind: MemoryRegionKind::Usable,
        start: 0x100,
        end: 0x4500,
    };
    let norm = normalize_region(&r).unwrap();
    assert_eq!(norm.0, 0x1000);
    assert_eq!(norm.1, 0x4000);
}

#[test]
fn normalize_region_too_small() {
    let r = MemoryRegion {
        kind: MemoryRegionKind::Usable,
        start: 0,
        end: 0x1000,
    };
    assert!(normalize_region(&r).is_some());

    let r2 = MemoryRegion {
        kind: MemoryRegionKind::Usable,
        start: 0x500,
        end: 0x1500,
    };
    assert!(normalize_region(&r2).is_none());
}

#[test]
fn allocator_single_region_basic() {
    let regions = [MemoryRegion {
        kind: MemoryRegionKind::Usable,
        start: 0,
        end: 2 * FRAME_SIZE,
    }];
    let mut alloc = PhysFrameAllocator::new();
    assert!(alloc.init(&regions));
    assert_eq!(alloc.total_frames(), 2);
    assert_eq!(alloc.free_frames(), 2);

    let f1 = alloc.allocate_frame().unwrap();
    assert_eq!(f1, 0);
    assert_eq!(alloc.allocated_frames(), 1);
    assert_eq!(alloc.free_frames(), 1);

    let f2 = alloc.allocate_frame().unwrap();
    assert_eq!(f2, FRAME_SIZE);
    assert_eq!(alloc.allocated_frames(), 2);
    assert_eq!(alloc.free_frames(), 0);

    assert!(alloc.allocate_frame().is_none());
    assert_eq!(alloc.allocated_frames(), 2);
}

#[test]
fn allocator_multiple_regions() {
    let regions = [
        MemoryRegion {
            kind: MemoryRegionKind::Usable,
            start: 0,
            end: FRAME_SIZE,
        },
        MemoryRegion {
            kind: MemoryRegionKind::Usable,
            start: 2 * FRAME_SIZE,
            end: 4 * FRAME_SIZE,
        },
    ];
    let mut alloc = PhysFrameAllocator::new();
    assert!(alloc.init(&regions));
    assert_eq!(alloc.total_frames(), 3);
    assert_eq!(alloc.usable_region_count(), 2);

    assert_eq!(alloc.allocate_frame().unwrap(), 0);
    assert_eq!(alloc.allocate_frame().unwrap(), 2 * FRAME_SIZE);
    assert_eq!(alloc.allocate_frame().unwrap(), 3 * FRAME_SIZE);
    assert!(alloc.allocate_frame().is_none());
}

#[test]
fn allocator_ignores_reserved() {
    let regions = [
        MemoryRegion {
            kind: MemoryRegionKind::Bootloader,
            start: 0,
            end: 10 * FRAME_SIZE,
        },
        MemoryRegion {
            kind: MemoryRegionKind::Usable,
            start: 0,
            end: FRAME_SIZE,
        },
    ];
    let mut alloc = PhysFrameAllocator::new();
    assert!(alloc.init(&regions));
    assert_eq!(alloc.total_frames(), 1);
    assert_eq!(alloc.usable_region_count(), 1);
    assert_eq!(alloc.allocate_frame().unwrap(), 0);
    assert!(alloc.allocate_frame().is_none());
}

#[test]
fn allocator_handles_unaligned_boundaries() {
    let regions = [MemoryRegion {
        kind: MemoryRegionKind::Usable,
        start: 0x100,
        end: 0x4500,
    }];
    let mut alloc = PhysFrameAllocator::new();
    assert!(alloc.init(&regions));
    assert_eq!(alloc.total_frames(), 3);
    assert_eq!(alloc.allocate_frame().unwrap(), 0x1000);
    assert_eq!(alloc.allocate_frame().unwrap(), 0x2000);
    assert_eq!(alloc.allocate_frame().unwrap(), 0x3000);
    assert!(alloc.allocate_frame().is_none());
}

#[test]
fn allocator_adjacent_regions() {
    let regions = [
        MemoryRegion {
            kind: MemoryRegionKind::Usable,
            start: 0,
            end: FRAME_SIZE,
        },
        MemoryRegion {
            kind: MemoryRegionKind::Usable,
            start: FRAME_SIZE,
            end: 2 * FRAME_SIZE,
        },
    ];
    let mut alloc = PhysFrameAllocator::new();
    assert!(alloc.init(&regions));
    assert_eq!(alloc.total_frames(), 2);
    assert_eq!(alloc.allocate_frame().unwrap(), 0);
    assert_eq!(alloc.allocate_frame().unwrap(), FRAME_SIZE);
    assert!(alloc.allocate_frame().is_none());
}

#[test]
fn allocator_gap_between_regions() {
    let regions = [
        MemoryRegion {
            kind: MemoryRegionKind::Usable,
            start: 0,
            end: FRAME_SIZE,
        },
        MemoryRegion {
            kind: MemoryRegionKind::Usable,
            start: 3 * FRAME_SIZE,
            end: 4 * FRAME_SIZE,
        },
    ];
    let mut alloc = PhysFrameAllocator::new();
    assert!(alloc.init(&regions));
    assert_eq!(alloc.total_frames(), 2);
    assert_eq!(alloc.allocate_frame().unwrap(), 0);
    assert_eq!(alloc.allocate_frame().unwrap(), 3 * FRAME_SIZE);
    assert!(alloc.allocate_frame().is_none());
}

#[test]
fn allocator_empty_map() {
    let regions: [MemoryRegion; 0] = [];
    let mut alloc = PhysFrameAllocator::new();
    assert!(alloc.init(&regions));
    assert_eq!(alloc.total_frames(), 0);
    assert!(alloc.allocate_frame().is_none());
}

#[test]
fn allocator_region_exactly_one_frame() {
    let regions = [MemoryRegion {
        kind: MemoryRegionKind::Usable,
        start: 0,
        end: FRAME_SIZE,
    }];
    let mut alloc = PhysFrameAllocator::new();
    assert!(alloc.init(&regions));
    assert_eq!(alloc.total_frames(), 1);
    assert_eq!(alloc.allocate_frame().unwrap(), 0);
    assert!(alloc.allocate_frame().is_none());
}
