//! Memory-map validation and reporting.
//!
//! The bootloader passes a list of physical memory regions. This is external
//! input and must be validated before the kernel trusts any of it.
//!
//! This module does NOT allocate memory. It only inspects the map, validates
//! it, and prints a controlled summary.

use crate::serial;
use bootloader_api::info::{MemoryRegion, MemoryRegionKind};

/// Summary statistics of the validated memory map.
///
/// All totals are computed with checked arithmetic; a map that overflows is
/// reported as an error and the caller must not trust the totals.
pub struct MemoryMapSummary {
    pub region_count: usize,
    pub usable_count: usize,
    pub usable_total: u64,
    pub reserved_total: u64,
    pub largest_usable: Option<MemoryRegion>,
    pub errors: MemoryMapErrors,
}

/// Problems found while validating the memory map.
#[derive(Default)]
pub struct MemoryMapErrors {
    pub empty: bool,
    pub too_many_regions: bool,
    pub invalid_region: bool,
    pub overlapping_regions: bool,
    pub overflow: bool,
}

impl MemoryMapErrors {
    pub fn is_clean(&self) -> bool {
        !self.empty
            && !self.too_many_regions
            && !self.invalid_region
            && !self.overlapping_regions
            && !self.overflow
    }
}

/// Validate the memory map and compute a summary.
///
/// # Safety
///
/// The caller must ensure `regions` points to a valid, aligned slice for the
/// duration of this call. The bootloader guarantees this for `BootInfo`.
pub unsafe fn validate_and_summarize(regions: &[MemoryRegion]) -> MemoryMapSummary {
    let mut summary = MemoryMapSummary {
        region_count: regions.len(),
        usable_count: 0,
        usable_total: 0,
        reserved_total: 0,
        largest_usable: None,
        errors: MemoryMapErrors::default(),
    };

    if regions.is_empty() {
        summary.errors.empty = true;
        return summary;
    }

    // Reject absurdly large maps so a corrupted count cannot drive a huge
    // iteration. The bootloader already bounds this, but treat it as external
    // input anyway.
    if regions.len() > 1_024 {
        summary.errors.too_many_regions = true;
        return summary;
    }

    // Validate each region and accumulate totals with checked arithmetic.
    for r in regions {
        // A region is invalid if start >= end.
        if r.start >= r.end {
            summary.errors.invalid_region = true;
            continue;
        }

        match r.kind {
            MemoryRegionKind::Usable => {
                summary.usable_count += 1;
                summary.usable_total = match summary.usable_total.checked_add(r.end - r.start) {
                    Some(v) => v,
                    None => {
                        summary.errors.overflow = true;
                        summary.usable_total
                    }
                };

                // Track the largest usable region.
                let size = r.end - r.start;
                let is_larger = match summary.largest_usable {
                    Some(l) => (l.end - l.start) < size,
                    None => true,
                };
                if is_larger {
                    summary.largest_usable = Some(*r);
                }
            }
            MemoryRegionKind::Bootloader => {
                summary.reserved_total = match summary.reserved_total.checked_add(r.end - r.start) {
                    Some(v) => v,
                    None => {
                        summary.errors.overflow = true;
                        summary.reserved_total
                    }
                };
            }
            // Unknown regions are neither usable nor reserved for accounting
            // purposes. They are reported separately by the caller.
            _ => {}
        }
    }

    // Check for overlapping usable regions. Overlap is only meaningful between
    // regions that the kernel might actually use, so compare every usable
    // region against every other usable region. This is O(n^2) but the map is
    // small (bounded above) and this runs once at boot.
    //
    // This runs directly on the input slice without allocation: the kernel is
    // #![no_std] and has no Vec at this stage.
    'outer: for i in 0..regions.len() {
        if !matches!(regions[i].kind, MemoryRegionKind::Usable) {
            continue;
        }
        for j in (i + 1)..regions.len() {
            if !matches!(regions[j].kind, MemoryRegionKind::Usable) {
                continue;
            }
            let a = &regions[i];
            let b = &regions[j];
            // Two half-open intervals [start, end) overlap when
            // a.start < b.end && b.start < a.end.
            if a.start < b.end && b.start < a.end {
                summary.errors.overlapping_regions = true;
                break 'outer;
            }
        }
    }

    summary
}

/// Print the validated memory-map summary to serial.
pub fn report_summary(summary: &MemoryMapSummary) {
    unsafe {
        serial::write_str("Elvaris kernel: --- memory map ---\r\n");

        serial::write_str("Elvaris kernel: region_count: ");
        serial::write_fmt(core::format_args!("{}", summary.region_count));
        serial::write_newline();

        serial::write_str("Elvaris kernel: usable_count: ");
        serial::write_fmt(core::format_args!("{}", summary.usable_count));
        serial::write_newline();

        serial::write_str("Elvaris kernel: usable_total: 0x");
        serial::write_fmt(core::format_args!("{:x}", summary.usable_total));
        serial::write_newline();

        serial::write_str("Elvaris kernel: reserved_total: 0x");
        serial::write_fmt(core::format_args!("{:x}", summary.reserved_total));
        serial::write_newline();

        if let Some(l) = summary.largest_usable {
            serial::write_str("Elvaris kernel: largest_usable: ");
            serial::write_fmt(core::format_args!(
                "[0x{:x}, 0x{:x}) size=0x{:x}",
                l.start,
                l.end,
                l.end - l.start
            ));
            serial::write_newline();
        } else {
            serial::write_str("Elvaris kernel: largest_usable: none\r\n");
        }

        if summary.errors.is_clean() {
            serial::write_str("Elvaris kernel: memory map: valid\r\n");
        } else {
            serial::write_str("Elvaris kernel: memory map: INVALID\r\n");
            if summary.errors.empty {
                serial::write_str("Elvaris kernel:   error: empty map\r\n");
            }
            if summary.errors.too_many_regions {
                serial::write_str("Elvaris kernel:   error: too many regions\r\n");
            }
            if summary.errors.invalid_region {
                serial::write_str("Elvaris kernel:   error: invalid region (start >= end)\r\n");
            }
            if summary.errors.overlapping_regions {
                serial::write_str("Elvaris kernel:   error: overlapping usable regions\r\n");
            }
            if summary.errors.overflow {
                serial::write_str("Elvaris kernel:   error: total overflow\r\n");
            }
        }

        serial::write_str("Elvaris kernel: --- end memory map ---\r\n");
    }
}