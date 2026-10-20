//! Shared helpers for the QEMU-based integration tests.
//!
//! The helpers discover the firmware and the bootable disk image, launch
//! QEMU, and capture the complete serial output. Capturing the output
//! completely matters: a test that only sees part of the log cannot tell a
//! missing marker from a marker that was cut off by the end of the pipe.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

/// Maximum time to wait for QEMU to finish before failing the test.
pub const TIMEOUT: Duration = Duration::from_secs(60);

/// Expected QEMU host exit code for a clean kernel boot.
///
/// The `isa-debug-exit` device encodes the byte the guest wrote as
/// `(byte << 1) | 1`, so the success code `0x10` becomes 33.
pub const EXPECTED_EXIT_CODE: i32 = 33;

/// Everything the test needs to know about a QEMU run.
pub struct QemuOutput {
    /// Host exit code of QEMU, or -1 if QEMU was killed or timed out.
    pub exit_code: i32,
    /// Complete serial output of the guest.
    pub stdout: String,
    /// Complete standard error output of QEMU.
    pub stderr: String,
}

impl QemuOutput {
    /// Whether the guest reported a successful boot.
    pub fn is_success(&self) -> bool {
        self.exit_code == EXPECTED_EXIT_CODE
    }

    /// A human-readable dump of the run, used in assertion messages.
    pub fn describe(&self) -> String {
        format!(
            "exit code: {}\n--- serial output ---\n{}\n--- qemu stderr ---\n{}",
            self.exit_code, self.stdout, self.stderr
        )
    }
}

/// Locate the `uefi.img` produced by the `os` build script.
///
/// The image lives under `target/<host-triple>/debug/build/os/<hash>/out/`.
/// We walk the `build/os` directories under the workspace target dir and
/// pick the most recent `uefi.img`, rather than hard-coding the hash.
pub fn find_uefi_image() -> Option<PathBuf> {
    let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target");

    let mut best: Option<(SystemTime, PathBuf)> = None;
    find_uefi_image_in(&target_dir, &mut best);
    best.map(|(_, path)| path)
}

fn find_uefi_image_in(dir: &Path, best: &mut Option<(SystemTime, PathBuf)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            find_uefi_image_in(&path, best);
        } else if path.file_name() == Some(std::ffi::OsStr::new("uefi.img")) {
            if let Ok(meta) = path.metadata() {
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                match &best {
                    Some((time, _)) if *time >= mtime => continue,
                    _ => *best = Some((mtime, path)),
                }
            }
        }
    }
}

/// Locate the OVMF firmware files.
pub fn ovmf_files() -> (PathBuf, PathBuf) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    (
        root.join("ovmf-x86_64/OVMF_CODE.fd"),
        root.join("ovmf-x86_64/OVMF_VARS.fd"),
    )
}

/// Launch QEMU with the given image and capture its complete output.
///
/// The output is drained by dedicated threads, so no serial output is lost
/// when the guest terminates the emulator.
pub fn run_qemu(image: &Path) -> std::io::Result<QemuOutput> {
    let (ovmf_code, ovmf_vars) = ovmf_files();

    let mut child: Child = Command::new("qemu-system-x86_64")
        .args([
            "-machine",
            "q35",
            "-m",
            "512",
            "-nographic",
            "-no-reboot",
            "-net",
            "none",
            "-drive",
            &format!(
                "if=pflash,unit=0,format=raw,file={},readonly=on",
                ovmf_code.display()
            ),
            "-drive",
            &format!("if=pflash,unit=1,format=raw,file={}", ovmf_vars.display()),
            "-drive",
            &format!("if=none,format=raw,file={},id=hd0", image.display()),
            "-device",
            "virtio-blk-pci,drive=hd0,bootindex=2",
            "-device",
            "isa-debug-exit,iobase=0x402,iosize=1",
            "-debugcon",
            "stdio",
            "-global",
            "isa-debugcon.iobase=0x403",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    let stdout_reader = std::thread::spawn(move || read_to_string(stdout));
    let stderr_reader = std::thread::spawn(move || read_to_string(stderr));

    let start = Instant::now();
    let mut exit_code = -1;
    loop {
        match child.try_wait()? {
            Some(status) => {
                exit_code = status.code().unwrap_or(-1);
                break;
            }
            None => {
                if start.elapsed() >= TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    exit_code = -1;
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();

    Ok(QemuOutput {
        exit_code,
        stdout,
        stderr,
    })
}

fn read_to_string(mut reader: impl Read) -> String {
    let mut buffer = Vec::new();
    let _ = reader.read_to_end(&mut buffer);
    String::from_utf8_lossy(&buffer).into_owned()
}

/// Assert that the guest emitted every marker and exited successfully.
pub fn assert_boot_success(output: &QemuOutput, markers: &[&str]) {
    assert!(
        output.is_success(),
        "QEMU did not report a successful boot; {}",
        output.describe()
    );
    for marker in markers {
        assert!(
            output.stdout.contains(marker),
            "serial output missing marker {:?}; {}",
            marker,
            output.describe()
        );
    }
}
