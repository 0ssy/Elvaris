//! Integration test: the kernel heap in QEMU.
//!
//! Boots the kernel and verifies that the heap was initialized, that its
//! pages were mapped through the virtual-memory subsystem, that the physical
//! frames came from the PMM, and that allocation, deallocation and
//! exhaustion behave as documented.

mod common;

use common::{assert_boot_success, find_uefi_image, run_qemu};

/// Markers that the kernel must emit for a working kernel heap.
const HEAP_MARKERS: &[&str] = &[
    // The physical-memory direct map is what makes the page tables reachable.
    "Elvaris kernel: physical_memory_offset: some(",
    // Heap initialization.
    "Elvaris kernel: heap: init ok",
    // The heap took its pages from the PMM.
    "Elvaris kernel: heap: pmm_frames=",
    "Elvaris kernel: heap: pmm_frames=262 ok",
    // The allocator itself.
    "Elvaris kernel: heap: self_test: alloc ok",
    "Elvaris kernel: heap: self_test: free ok",
    "Elvaris kernel: heap: self_test: exhaustion ok",
    "Elvaris kernel: heap: self_test ok",
    // The provisional heap layout.
    "Elvaris kernel: heap: base=0xffffa00000000000",
    "reserved_bytes=67108864 mapped_pages=256",
    "Elvaris kernel: heap: frames=0x",
    // The allocator statistics: a 1 MiB heap, completely free again after
    // the self test.
    "Elvaris kernel: heap: capacity=1048576 used=0 free=1048544 blocks=1",
    "Elvaris kernel: --- end heap ---",
];

#[test]
fn heap_initializes_maps_pages_and_allocates() {
    let image = find_uefi_image()
        .expect("uefi.img not found under target/ — build the `os` package first");

    let output = run_qemu(image.as_path()).expect("failed to launch QEMU");

    assert_boot_success(&output, HEAP_MARKERS);
}

#[test]
fn heap_report_follows_the_boot_report() {
    let image = find_uefi_image()
        .expect("uefi.img not found under target/ — build the `os` package first");

    let output = run_qemu(image.as_path()).expect("failed to launch QEMU");

    // The heap report has to appear after the boot information and before the
    // end of the boot: a heap that is reported before the memory map would
    // mean the heap was built on unvalidated memory.
    let bootinfo = output
        .stdout
        .find("Elvaris kernel: --- BootInfo ---")
        .expect("BootInfo report missing");
    let memory_map = output
        .stdout
        .find("Elvaris kernel: memory map: valid")
        .expect("memory map report missing");
    let heap = output
        .stdout
        .find("Elvaris kernel: heap: init ok")
        .expect("heap initialization marker missing");
    let heap_end = output
        .stdout
        .find("Elvaris kernel: --- end heap ---")
        .expect("heap report end marker missing");

    assert!(bootinfo < memory_map, "the memory map must follow the boot info");
    assert!(memory_map < heap, "the heap must be initialized after the memory map");
    assert!(heap < heap_end, "the heap report must be closed");
}
