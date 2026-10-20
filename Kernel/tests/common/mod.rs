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

/// Name of the emulator executable.
const QEMU_PROGRAM: &str = "qemu-system-x86_64";

/// Directories the emulator is installed in when it is not on the path.
///
/// On Windows the installer puts the emulator in `C:\Program Files\qemu` and
/// does not add it to the path; on the other systems the usual locations are
/// enough.
const QEMU_DIRECTORIES: &[&str] = &[
    "C:\\Program Files\\qemu",
    "C:\\Program Files (x86)\\qemu",
    "/usr/bin",
    "/usr/local/bin",
    "/opt/homebrew/bin",
];

/// Locate the emulator.
///
/// The `ELVARIS_QEMU` environment variable wins, then the path, then the
/// well-known installation directories. The search makes the integration tests
/// work on a machine where the emulator is installed but not on the path.
pub fn qemu_program() -> PathBuf {
    if let Ok(path) = std::env::var("ELVARIS_QEMU") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path;
        }
    }

    if let Some(path) = find_on_path(QEMU_PROGRAM) {
        return path;
    }

    let candidate = Path::new(r"C:\Program Files\qemu").join(executable_name(QEMU_PROGRAM));
    if candidate.is_file() {
        return candidate;
    }

    for directory in QEMU_DIRECTORIES {
        if *directory == r"C:\Program Files\qemu" {
            continue;
        }
        let candidate = Path::new(directory).join(executable_name(QEMU_PROGRAM));
        if candidate.is_file() {
            return candidate;
        }
    }

    // Nothing was found: the name is returned unchanged, so that the error
    // the caller sees names the program that is missing.
    PathBuf::from(executable_name(QEMU_PROGRAM))
}

/// The name of the executable on this platform.
fn executable_name(program: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", program)
    } else {
        program.to_string()
    }
}

/// Search the path for an executable.
fn find_on_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let name = executable_name(program);
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(&name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Launch QEMU with the given image and capture its complete output.
///
/// The output is drained by dedicated threads, so no serial output is lost
/// when the guest terminates the emulator.
pub fn run_qemu(image: &Path) -> std::io::Result<QemuOutput> {
    let (ovmf_code, ovmf_vars) = ovmf_files();

    let mut child: Child = Command::new(qemu_program())
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
