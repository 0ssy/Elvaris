//! Integration test: the user mode in QEMU.
//!
//! Boots the kernel and verifies that the kernel enters ring 3, that the user
//! program runs, that its output reaches the serial console, that it exits
//! cleanly, and that the kernel survives a user program that faults and one
//! that raises a system call the kernel does not know.

mod common;

use common::{assert_boot_success, find_uefi_image, run_qemu};

/// Markers the kernel must emit for a working user mode.
const USER_MARKERS: &[&str] = &[
    // The address space the kernel built for the user program.
    "Elvaris kernel: user: address space: start=0x8000000000 end=0x8000006000",
    "Elvaris kernel: user: region: base=0x8000000000 pages=1",
    "Elvaris kernel: user: region: base=0x8000001000 pages=1",
    "Elvaris kernel: user: region: base=0x8000002000 pages=4",
    // The transition into ring 3.
    "Elvaris kernel: user: starting the user program",
    "Elvaris kernel: user: entry=0x8000000000 stack_top=0x8000006000",
    // The output of the user program itself.
    "Hello from user space!",
    // The clean end of the user program.
    "Elvaris kernel: user: exit: code=0",
    "Elvaris kernel: user: outcome=exited code=0",
    "Elvaris kernel: user: syscalls=2",
    // The self test: a user program that faults and one that raises an
    // unknown system call are both taken down, and the kernel survives.
    "Elvaris kernel: user: fault in user program",
    "Elvaris kernel: user: fault: vector=14 name=Page Fault",
    "Elvaris kernel: user: self_test: fault probe ok",
    "Elvaris kernel: user: invalid system call: number=99",
    "Elvaris kernel: user: self_test: unknown syscall probe ok",
    "Elvaris kernel: user: self_test ok",
    "Elvaris kernel: --- end user ---",
];

/// Markers that must not appear: a user fault is not a kernel fault.
const FORBIDDEN_MARKERS: &[&str] = &[
    "Elvaris kernel: EXCEPTION",
    "Elvaris kernel: PANIC",
    "Elvaris kernel: user: self_test: fault probe failed",
    "Elvaris kernel: user: self_test: unknown syscall probe failed",
];

#[test]
fn the_user_program_runs_writes_and_exits() {
    let image =
        find_uefi_image().expect("uefi.img not found under target/ — build the `os` package first");

    let output = run_qemu(image.as_path()).expect("failed to launch QEMU");

    assert_boot_success(&output, USER_MARKERS);

    for marker in FORBIDDEN_MARKERS {
        assert!(
            !output.stdout.contains(marker),
            "the serial output must not contain {:?}; {}",
            marker,
            output.describe()
        );
    }
}

#[test]
fn the_user_output_comes_before_the_end_of_the_user_section() {
    let image =
        find_uefi_image().expect("uefi.img not found under target/ — build the `os` package first");

    let output = run_qemu(image.as_path()).expect("failed to launch QEMU");

    let start = output
        .stdout
        .find("Elvaris kernel: user: starting the user program")
        .expect("the kernel did not report the start of the user program");
    let message = output
        .stdout
        .find("Hello from user space!")
        .expect("the user program did not reach the console");
    let exit = output
        .stdout
        .find("Elvaris kernel: user: exit: code=0")
        .expect("the user program did not exit");
    let end = output
        .stdout
        .find("Elvaris kernel: --- end user ---")
        .expect("the kernel did not close the user section");

    assert!(
        start < message,
        "the output has to follow the start of the program"
    );
    assert!(
        message < exit,
        "the exit has to follow the output of the program"
    );
    assert!(
        exit < end,
        "the user section has to be closed after the exit"
    );
}
