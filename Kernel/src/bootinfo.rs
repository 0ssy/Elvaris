//! Inspection of the `BootInfo` structure handed to the kernel by the
//! `bootloader` crate.
//!
//! The bootloader data is external input. This module does not trust any
//! field implicitly; it reports what is actually present under the current
//! UEFI configuration so the kernel can decide what to rely on.

use crate::serial;
use bootloader_api::info::{BootInfo, MemoryRegionKind, Optional};

/// Print a human-readable report of the BootInfo fields that are available
/// under the current bootloader configuration.
///
/// Fields are reported even when they are `None`/absent so the report is a
/// complete inventory rather than a filtered view.
pub fn report(boot_info: &BootInfo) {
    unsafe {
        serial::write_str("Elvaris kernel: --- BootInfo ---\r\n");
        serial::write_str("Elvaris kernel: api_version: ");
        serial::write_fmt(core::format_args!(
            "{}.{}.{}{}",
            boot_info.api_version.version_major(),
            boot_info.api_version.version_minor(),
            boot_info.api_version.version_patch(),
            if boot_info.api_version.pre_release() { "-pre" } else { "" }
        ));
        serial::write_newline();
        serial::write_str("Elvaris kernel: memory_regions: ");
        serial::write_fmt(core::format_args!(
            "{} entries",
            boot_info.memory_regions.len()
        ));
        serial::write_newline();
        serial::write_str("Elvaris kernel: kernel_addr: ");
        serial::write_fmt(core::format_args!("0x{:x}", boot_info.kernel_addr));
        serial::write_newline();

        serial::write_str("Elvaris kernel: kernel_len: ");
        serial::write_fmt(core::format_args!("0x{:x}", boot_info.kernel_len));
        serial::write_newline();

        serial::write_str("Elvaris kernel: kernel_stack_bottom: ");
        serial::write_fmt(core::format_args!("0x{:x}", boot_info.kernel_stack_bottom));
        serial::write_newline();

        serial::write_str("Elvaris kernel: kernel_stack_len: ");
        serial::write_fmt(core::format_args!("0x{:x}", boot_info.kernel_stack_len));
        serial::write_newline();

        report_optional(
            "physical_memory_offset",
            &boot_info.physical_memory_offset,
        );
        report_optional(
            "recursive_index",
            &boot_info.recursive_index,
        );
        report_optional_u64("rsdp_addr", &boot_info.rsdp_addr);
        report_optional("tls_template", &boot_info.tls_template);
        report_optional_u64("ramdisk_addr", &boot_info.ramdisk_addr);

        serial::write_str("Elvaris kernel: ramdisk_len: ");
        serial::write_fmt(core::format_args!("0x{:x}", boot_info.ramdisk_len));
        serial::write_newline();

        // Framebuffer.
        match boot_info.framebuffer.as_ref() {
            Some(fb) => {
                serial::write_str("Elvaris kernel: framebuffer: present\r\n");
                let info = fb.info();
                serial::write_str("Elvaris kernel: framebuffer info: ");
                serial::write_fmt(core::format_args!(
                    "{}x{} stride={} bpp={}",
                    info.width,
                    info.height,
                    info.stride,
                    info.bytes_per_pixel
                ));
                serial::write_newline();
            }
            None => {
                serial::write_str("Elvaris kernel: framebuffer: absent\r\n");
            }
        }

        serial::write_str("Elvaris kernel: --- end BootInfo ---\r\n");
    }
}

/// Report an optional value that implements `Into<Option<T>>`.
fn report_optional<T: core::fmt::Debug>(name: &str, value: &Optional<T>) {
    unsafe {
        serial::write_str("Elvaris kernel: ");
        serial::write_str(name);
        serial::write_str(": ");
        match value.as_ref() {
            Some(v) => {
                serial::write_str("some(");
                serial::write_fmt(core::format_args!("{:?}", v));
                serial::write_str(")");
            }
            None => serial::write_str("none"),
        }
        serial::write_newline();
    }
}

/// Report an optional u64 as a hex address.
fn report_optional_u64(name: &str, value: &Optional<u64>) {
    unsafe {
        serial::write_str("Elvaris kernel: ");
        serial::write_str(name);
        serial::write_str(": ");
        match value.as_ref() {
            Some(v) => {
                serial::write_str("0x");
                serial::write_fmt(core::format_args!("{:x}", v));
            }
            None => serial::write_str("none"),
        }
        serial::write_newline();
    }
}

/// Count memory regions by kind.
///
/// Returns a small struct so the caller can decide what to do with the counts
/// without re-iterating the map.
pub struct MemoryRegionCounts {
    pub usable: usize,
    pub bootloader: usize,
    pub unknown: usize,
}

impl MemoryRegionCounts {
    pub fn from_regions(regions: &[bootloader_api::info::MemoryRegion]) -> Self {
        let mut counts = MemoryRegionCounts {
            usable: 0,
            bootloader: 0,
            unknown: 0,
        };

        for r in regions {
            match r.kind {
                MemoryRegionKind::Usable => counts.usable += 1,
                MemoryRegionKind::Bootloader => counts.bootloader += 1,
                MemoryRegionKind::UnknownUefi(_) | MemoryRegionKind::UnknownBios(_) => {
                    counts.unknown += 1;
                }
                // `MemoryRegionKind` is `#[non_exhaustive]`; future bootloader
                // versions may add new variants. Treat anything unexpected as
                // unknown rather than refusing to compile.
                _ => counts.unknown += 1,
            }
        }

        counts
    }
}