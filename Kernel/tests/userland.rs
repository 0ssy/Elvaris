//! Integration test: the userland of Elvaris in QEMU.
//!
//! The test boots the kernel, waits for the shell of the userland, and
//! drives the shell the way a person at the keyboard would. It asserts:
//!
//! - the kernel mounts the filesystem and starts `/init`,
//! - the shell starts, shows its prompt, and answers the built-in commands,
//! - the shell finds a program of the filesystem and runs it,
//! - the shell reports the exit status of that program,
//! - the shell reports the commands it does not know,
//! - the machine stops when the shell is told to stop, and QEMU reports the
//!   exit code of the guest.

mod common;

use common::{assert_boot_success, find_uefi_image, run_qemu_session};

/// The commands the test sends to the shell.
const COMMANDS: &[&str] = &[
    "help",
    "pwd",
    "ls",
    "ls /bin",
    "echo hello world",
    "ps",
    "mem",
    "cat /nope",
    "nosuchcommand",
    "hello",
    "clear",
    "exit",
];

/// Markers the machine must show for a working userland.
const REQUIRED_MARKERS: &[&str] = &[
    // The filesystem and the first program of the machine.
    "Elvaris kernel: storage: the filesystem is mounted",
    "Elvaris kernel: user: starting the userland: program=/init",
    // The shell of the userland.
    "Elvaris shell. Type help for the commands.",
    // The built-in commands.
    "commands: help pwd ls cat echo ps mem clear shutdown reboot exit",
    "Elvaris kernel: ps: pid state name",
    "Elvaris kernel: ps: 1 running /init",
    "Elvaris kernel: ps: 2 running /bin/sh",
    "Elvaris kernel: mem: frames total=",
    "Elvaris kernel: mem: heap capacity=",
    // A program of the filesystem, started by the shell.
    "Hello from the Elvaris userland!",
    "exit status: 0",
    // The commands that do not work.
    "sh: the command was not found",
    "sh: the file could not be read",
    // The end of the userland.
    "Elvaris kernel: user: the userland ended",
];

/// Markers that must not appear: a fault of the userland is not a fault of
/// the kernel.
const FORBIDDEN_MARKERS: &[&str] = &[
    "Elvaris kernel: EXCEPTION",
    "Elvaris kernel: PANIC",
    "Elvaris kernel: user: the userland could not run",
    "Elvaris kernel: user: invalid pointer",
];

#[test]
fn the_shell_runs_the_commands_of_the_user() {
    let image =
        find_uefi_image().expect("uefi.img not found under target/ — build the `os` package first");

    let output = run_qemu_session(image.as_path(), COMMANDS).expect("failed to launch QEMU");

    assert_boot_success(&output, REQUIRED_MARKERS);

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
fn the_shell_answers_the_built_in_commands() {
    let image =
        find_uefi_image().expect("uefi.img not found under target/ — build the `os` package first");

    let output = run_qemu_session(image.as_path(), COMMANDS).expect("failed to launch QEMU");

    // The answer of `pwd` is the root of the filesystem.
    assert!(
        output.stdout.contains("sh> pwd/"),
        "the shell did not answer pwd with the root; {}",
        output.describe()
    );

    // The answer of `echo` is the text of its argument.
    assert!(
        output.stdout.contains("sh> echo hello worldhello world"),
        "the shell did not answer echo with its argument; {}",
        output.describe()
    );

    // The answer of `ls` holds the entries of the root, the answer of
    // `ls /bin` holds the programs of the filesystem.
    assert!(
        output.stdout.contains("finit"),
        "the listing of the root does not hold the init program; {}",
        output.describe()
    );
    assert!(
        output.stdout.contains("fhello"),
        "the listing of /bin does not hold the hello program; {}",
        output.describe()
    );
}

#[test]
fn the_shell_reports_the_exit_status_of_a_program() {
    let image =
        find_uefi_image().expect("uefi.img not found under target/ — build the `os` package first");

    let output = run_qemu_session(image.as_path(), &["hello"]).expect("failed to launch QEMU");

    let hello = output
        .stdout
        .find("Hello from the Elvaris userland!")
        .expect("the shell did not start the hello program");
    let status = output
        .stdout
        .find("exit status: 0")
        .expect("the shell did not report the exit status of the program");

    assert!(
        hello < status,
        "the status has to follow the output of the program; {}",
        output.describe()
    );
}
