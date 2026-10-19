//! Virtual and physical address utilities.
//!
//! For Milestone 4, we document and implement basic address translation
//! helpers. We must not assume identity mapping; we rely on the bootloader's
//! physical-memory offset if available.

#![allow(dead_code)]

use bootloader_api::info::Optional;

/// Convert a physical address to a virtual address using the physical memory offset.
/// 
/// Returns None if the offset is not available. The caller must ensure the
/// offset is valid.
pub fn phys_to_virt(phys: u64, offset: &Optional<u64>) -> Option<u64> {
    match offset.as_ref() {
        Some(off) => Some(phys + *off),
        None => None,
    }
}

/// Convert a virtual address to a physical address using the physical memory offset.
/// 
/// Returns None if the offset is not available.
pub fn virt_to_phys(virt: u64, offset: &Optional<u64>) -> Option<u64> {
    match offset.as_ref() {
        Some(off) => virt.checked_sub(*off),
        None => None,
    }
}
