//! Integration test: boot the Elvaris kernel in QEMU and verify the serial
//! markers and exit code.
//!
//! The test runs on the host target and links against the standard library.
//! It discovers the `uefi.img` produced by the `os` build script, launches
//! QEMU with the OVMF firmware, and asserts:
//!
//! - QEMU exits with code 33 (`(0x10 << 1) | 1` from `isa-debug-exit`).
//! - The serial output contains every required marker emitted by the kernel.

mod common;

use common::{EXPECTED_EXIT_CODE, assert_boot_success, find_uefi_image, run_qemu};

/// Serial markers the kernel must emit on a successful boot.
const REQUIRED_MARKERS: &[&str] = &[
    "Elvaris kernel: started on x86-64",
    "Elvaris kernel: boot information received",
    "Elvaris kernel: usable memory regions:",
    "Elvaris kernel: --- BootInfo ---",
    "Elvaris kernel: --- end BootInfo ---",
    "Elvaris kernel: --- memory map ---",
    "Elvaris kernel: memory map: valid",
    "Elvaris kernel: pmm: usable_regions=",
    "Elvaris kernel: --- end memory map ---",
];

#[test]
fn boot_exits_with_success_code_and_markers() {
    let image =
        find_uefi_image().expect("uefi.img not found under target/ — build the `os` package first");

    let output = run_qemu(image.as_path()).expect("failed to launch QEMU");

    assert_eq!(
        output.exit_code,
        EXPECTED_EXIT_CODE,
        "QEMU exited with code {} (expected {}); {}",
        output.exit_code,
        EXPECTED_EXIT_CODE,
        output.describe()
    );

    assert_boot_success(&output, REQUIRED_MARKERS);
}
