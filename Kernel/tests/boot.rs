//! Integration test: boot the Elvaris kernel in QEMU and verify the serial
//! markers and exit code.
//!
//! The test runs on the host target and links against the standard library.
//! It discovers the `uefi.img` produced by the `os` build script, launches
//! QEMU with the OVMF firmware, and asserts:
//!
//! - QEMU exits with code 33 (`(0x10 << 1) | 1` from `isa-debug-exit`).
//! - The serial output contains every required marker emitted by the kernel.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// Serial markers the kernel must emit on a successful boot.
const REQUIRED_MARKERS: &[&str] = &[
    "Elvaris kernel: started on x86-64",
    "Elvaris kernel: boot information received",
    "Elvaris kernel: usable memory regions:",
    "Elvaris kernel: --- BootInfo ---",
    "Elvaris kernel: --- end BootInfo ---",
    "Elvaris kernel: --- memory map ---",
    "Elvaris kernel: memory map: valid",
    "Elvaris kernel: --- end memory map ---",
];

/// Expected QEMU host exit code for a clean kernel boot.
const EXPECTED_EXIT_CODE: i32 = 33;

/// Maximum time to wait for QEMU to finish before failing the test.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Locate the `uefi.img` produced by the `os` build script.
///
/// The image lives under `target/<host-triple>/debug/build/os/<hash>/out/`.
/// We walk the `build/os` directories under the workspace target dir and
/// pick the most recent `uefi.img`, rather than hard-coding the hash.
fn find_uefi_image() -> Option<PathBuf> {
    let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target");

    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;

    find_uefi_image_in(&target_dir, &mut best);

    best.map(|(_, p)| p)
}

fn find_uefi_image_in(
    dir: &std::path::Path,
    best: &mut Option<(std::time::SystemTime, PathBuf)>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            find_uefi_image_in(&path, best);
        } else if path.file_name() == Some(std::ffi::OsStr::new("uefi.img")) {
            if let Ok(meta) = path.metadata() {
                let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                match &best {
                    Some((t, _)) if *t >= mtime => continue,
                    _ => *best = Some((mtime, path)),
                }
            }
        }
    }
}

/// Locate the OVMF firmware files, falling back to a relative path.
fn ovmf_dirs() -> (PathBuf, PathBuf) {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("..");
    let code = root.join("ovmf-x86_64/OVMF_CODE.fd");
    let vars = root.join("ovmf-x86_64/OVMF_VARS.fd");
    (code, vars)
}

/// Launch QEMU with the given image, capture stdout, and wait for exit.
///
/// Uses `try_wait` polling with a sleep loop so the timeout is
/// cross-platform (no `wait_with_timeout` dependency).
fn run_qemu(image: &std::path::Path) -> std::io::Result<(i32, String)> {
    let (ovmf_code, ovmf_vars) = ovmf_dirs();

    let mut child = Command::new("qemu-system-x86_64")
        .args([
            "-machine", "q35",
            "-m", "512",
            "-nographic",
            "-no-reboot",
            "-net", "none",
            "-drive",
            &format!("if=pflash,unit=0,format=raw,file={},readonly=on", ovmf_code.display()),
            "-drive",
            &format!("if=pflash,unit=1,format=raw,file={}", ovmf_vars.display()),
            "-drive",
            &format!("if=none,format=raw,file={},id=hd0", image.display()),
            "-device", "virtio-blk-pci,drive=hd0,bootindex=2",
            "-device", "isa-debug-exit,iobase=0x402,iosize=1",
            "-debugcon", "stdio",
            "-global", "isa-debugcon.iobase=0x403",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    let mut output = String::new();
    let start = std::time::Instant::now();

    loop {
        // Check for exit first.
        match child.try_wait() {
            Ok(Some(status)) => {
                if let Some(code) = status.code() {
                    return Ok((code, output));
                }
                // Killed by signal — treat as failure.
                return Ok((-1, output));
            }
            Ok(None) => {}
            Err(e) => return Err(e),
        }

        if start.elapsed() >= TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Ok((-1, output));
        }

        // Drain any available stdout without blocking.
        if let Some(stdout) = child.stdout.as_mut() {
            use std::io::Read;
            let mut buf = [0u8; 4096];
            match stdout.read(&mut buf) {
                Ok(0) => {}
                Ok(n) => {
                    output.push_str(&String::from_utf8_lossy(&buf[..n]));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn boot_exits_with_success_code_and_markers() {
    let image = find_uefi_image()
        .expect("uefi.img not found under target/ — build the `os` package first");

    let (code, output) = run_qemu(image.as_path())
        .expect("failed to launch QEMU");

    assert_eq!(
        code, EXPECTED_EXIT_CODE,
        "QEMU exited with code {} (expected {}); serial output:\n{}",
        code, EXPECTED_EXIT_CODE, output
    );

    for marker in REQUIRED_MARKERS {
        assert!(
            output.contains(marker),
            "serial output missing marker {:?}; output:\n{}",
            marker,
            output
        );
    }
}