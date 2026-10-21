//! Shared helpers for the QEMU-based integration tests.
//!
//! The helpers discover the firmware and the bootable disk image, launch
//! QEMU, and capture the complete serial output. Capturing the output
//! completely matters: a test that only sees part of the log cannot tell a
//! missing marker from a marker that was cut off by the end of the pipe.
//!
//! # The serial console
//!
//! The serial console of the guest is a TCP socket, and the test is the
//! client of that socket. The kernel hands the console of the userland to
//! the shell, and the shell waits for the person at the keyboard, so a test
//! that wants the machine to stop has to tell the shell to stop. The socket
//! is also the way the test checks what the shell answered.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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

/// The prompt the shell shows when it waits for a command.
pub const SHELL_PROMPT: &str = "sh> ";

/// Everything the test needs to know about a QEMU run.
pub struct QemuOutput {
    /// Host exit code of QEMU, or -1 if QEMU was killed or timed out.
    pub exit_code: i32,
    /// Complete serial output of the guest.
    pub stdout: String,
    /// Complete standard error output of QEMU.
    pub stderr: String,
    /// The output of the debug console of the machine.
    pub debug: String,
}

impl QemuOutput {
    /// Whether the guest reported a successful boot.
    pub fn is_success(&self) -> bool {
        self.exit_code == EXPECTED_EXIT_CODE
    }

    /// A human-readable dump of the run, used in assertion messages.
    pub fn describe(&self) -> String {
        format!(
            "exit code: {}\n--- serial output ---\n{}\n--- debug console ---\n{}\n--- qemu stderr ---\n{}",
            self.exit_code, self.stdout, self.debug, self.stderr
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

/// The size of the data disk the tests use, in bytes.
///
/// The disk holds the filesystem of the machine, which is small: the
/// programs of the userland and the files of the person at the console.
pub const DATA_DISK_SIZE: u64 = 64 * 1024;

/// Create an empty data disk for the guest.
///
/// The disk is a plain file of the given size. The guest writes its
/// filesystem into it, and reads it back on the next boot.
pub fn create_data_disk(path: &Path, size: u64) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    file.set_len(size)?;
    Ok(())
}

/// A directory the test may write into, removed when the test ends.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Create a directory with a fresh name.
    pub fn new(name: &str) -> std::io::Result<Self> {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|since| since.as_nanos())
            .unwrap_or(0);
        path.push(format!("elvaris-{name}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&path)?;
        Ok(TempDir { path })
    }

    /// The path of the directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The path of a file inside the directory.
    pub fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
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
    run_qemu_session(image, &[])
}

/// Launch QEMU, drive the shell of the guest, and capture the output.
///
/// The commands are sent to the shell of the guest one after the other: the
/// test waits for the prompt of the shell before it sends the next command,
/// so the answer of a command is never mixed with the answer of the next
/// one. The shell is told to stop at the end, so that the machine ends the
/// boot on its own and QEMU reports the exit code of the guest.
pub fn run_qemu_session(image: &Path, commands: &[&str]) -> std::io::Result<QemuOutput> {
    run_qemu_session_with_disk(image, None, commands)
}

/// Launch QEMU with a data disk, `drive` the shell of the guest, and capture
/// the output.
///
/// The data disk is attached to the guest as a second block device. The
/// guest uses it to keep its files between two boots.
pub fn run_qemu_session_with_disk(
    image: &Path,
    disk: Option<&Path>,
    commands: &[&str],
) -> std::io::Result<QemuOutput> {
    let (ovmf_code, ovmf_vars) = ovmf_files();

    // The test owns the port of the serial console, so that the guest
    // connects to a socket that is already there.
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();

    let mut command = Command::new(qemu_program());
    command.args([
        "-machine",
        "q35",
        "-m",
        "512",
        "-nographic",
        "-monitor",
        "none",
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
    ]);
    if let Some(disk) = disk {
        command.args([
            "-drive",
            &format!("if=none,format=raw,file={},id=data0", disk.display()),
            "-device",
            "virtio-blk-pci,drive=data0",
        ]);
    }
    command.args([
        "-device",
        "isa-debug-exit,iobase=0x402,iosize=1",
        "-debugcon",
        "stdio",
        "-global",
        "isa-debugcon.iobase=0x403",
        "-serial",
        &format!("tcp:127.0.0.1:{port}"),
    ]);

    let mut child: Child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    let stdout_reader = std::thread::spawn(move || read_to_string(stdout));
    let stderr_reader = std::thread::spawn(move || read_to_string(stderr));

    // The guest connects to the serial console as soon as it starts.
    listener.set_nonblocking(false)?;
    let mut console = match listener.accept() {
        Ok((stream, _)) => stream,
        Err(error) => {
            let _ = child.kill();
            return Err(error);
        }
    };
    console.set_read_timeout(Some(Duration::from_millis(200)))?;

    let mut serial = String::new();
    // The shell shows its prompt when it waits for a command.
    let mut prompts = wait_for_prompt(&mut console, &mut serial, 0, &mut child);

    for command in commands {
        writeln!(console, "{command}")?;
        console.flush()?;
        prompts = wait_for_prompt(&mut console, &mut serial, prompts, &mut child);
    }

    // The shell stops, and with it the machine.
    if prompts > 0 && child.try_wait().map(|status| status.is_none()).unwrap_or(true) {
        let _ = writeln!(console, "exit");
        let _ = console.flush();
    }
    drain(&mut console, &mut serial);

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

    let debug = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();

    Ok(QemuOutput {
        exit_code,
        stdout: serial,
        stderr,
        debug,
    })
}

/// Read from the console until the shell shows a new prompt.
///
/// The function returns the number of the prompts the shell has shown so
/// far. The wait also ends when the machine stops: a command that ends the
/// machine never shows a new prompt.
fn wait_for_prompt(
    console: &mut TcpStream,
    serial: &mut String,
    seen: usize,
    child: &mut Child,
) -> usize {
    let start = Instant::now();
    loop {
        drain(console, serial);
        let prompts = count_prompts(serial);
        if prompts > seen {
            return prompts;
        }
        if child.try_wait().ok().flatten().is_some() {
            return prompts;
        }
        if start.elapsed() >= TIMEOUT {
            return prompts;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Read whatever the guest has sent, without waiting for more.
fn drain(console: &mut TcpStream, serial: &mut String) {
    let mut buffer = [0u8; 4096];
    loop {
        match console.read(&mut buffer) {
            Ok(0) => return,
            Ok(count) => {
                serial.push_str(&String::from_utf8_lossy(&buffer[..count]));
            }
            Err(_) => return,
        }
    }
}

/// The number of the prompts of the shell in the output.
fn count_prompts(serial: &str) -> usize {
    serial.matches(SHELL_PROMPT).count()
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
