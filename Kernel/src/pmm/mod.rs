//! Physical memory management (PMM).
//!
//! This module wires up frame abstraction and physical frame allocation.

use spin::Mutex;

pub mod frame;
pub mod phys_alloc;

#[cfg(test)]
mod tests;

static PMM_ALLOCATOR: Mutex<Option<phys_alloc::PhysFrameAllocator>> = Mutex::new(None);

pub fn init_from_memory_map(regions: &[bootloader_api::info::MemoryRegion]) -> bool {
    let mut alloc = phys_alloc::PhysFrameAllocator::new();
    let success = alloc.init(regions);
    *PMM_ALLOCATOR.lock() = Some(alloc);
    success
}

pub fn with_allocator<F, R>(f: F) -> R
where
    F: FnOnce(&mut phys_alloc::PhysFrameAllocator) -> R,
{
    let mut guard = PMM_ALLOCATOR.lock();
    if let Some(ref mut alloc) = *guard {
        f(alloc)
    } else {
        panic!("PMM not initialized")
    }
}

pub fn reserve_frame(addr: u64) {
    with_allocator(|alloc| alloc.reserve_frame(addr));
}

