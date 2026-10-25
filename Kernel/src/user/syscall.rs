//! The system-call ABI.
//!
//! A user program reaches the kernel through one instruction: `int 0x80`. The
//! interrupt descriptor table entry of that vector is the only entry the
//! processor accepts from user mode, so a user program cannot raise any
//! other interrupt.
//!
//! # The registers
//!
//! The registers follow the System V convention:
//!
//! | Register | Meaning                                               |
//! |----------|-------------------------------------------------------|
//! | `RAX`    | the number of the system call                          |
//! | `RDI`    | the first argument                                    |
//! | `RSI`    | the second argument                                   |
//! | `RDX`    | the third argument                                    |
//! | `RCX`    | the fourth argument                                   |
//! | `RAX`    | the return value                                      |
//!
//! The arguments are untrusted. Every pointer a program passes is validated
//! against the user address space and against the page tables before the
//! kernel dereferences it.
//!
//! # The calls
//!
//! | Number | Call | Meaning |
//! |--------|------|---------|
//! | 1 | `write(fd, buffer, length)` | write to the console or to a file |
//! | 2 | `exit(code)` | end the program |
//! | 3 | `open(path, length, flags)` | open a file |
//! | 4 | `read(fd, buffer, length)` | read a line or read a file |
//! | 5 | `getdents(path, length, buffer, length)` | list a directory |
//! | 6 | `close(fd)` | close a file |
//! | 7 | `stat(path, length, buffer)` | read the metadata of an object |
//! | 8 | `exec(path, length, arguments, length)` | run a program |
//! | 9 | `meminfo()` | report the memory of the machine |
//! | 10 | `procs()` | report the processes of the machine |
//! | 11 | `shutdown()` | stop the machine |
//! | 12 | `reboot()` | stop the machine |
//! | 13 | `format_number(value, buffer, length)` | render a number |
//!
//! # The result
//!
//! A call that succeeds returns the number it produced: the number of bytes
//! written, the number of the file descriptor, the number of the exit code
//! of the program that ran. A call that fails returns a negative number,
//! whose magnitude is the number of the error.

use alloc::string::String;
use alloc::vec::Vec;

use crate::console::Console;
use crate::cpu::vectors::InterruptFrame;
use crate::fs::{FileHandle, FileSystem, FileType, FsError, OpenFlags};
use crate::process::{self, ExitStatus};
use crate::serial;
use crate::storage;
use crate::sysinfo;
use crate::user::exec;
use crate::user::loader::LoadError;
use crate::user::pointer;
use crate::user::session;

/// The number of the `write` system call.
pub const SYS_WRITE: u64 = 1;

/// The number of the `exit` system call.
pub const SYS_EXIT: u64 = 2;

/// The number of the `open` system call.
pub const SYS_OPEN: u64 = 3;

/// The number of the `read` system call.
pub const SYS_READ: u64 = 4;

/// The number of the `getdents` system call.
pub const SYS_GETDENTS: u64 = 5;

/// The number of the `close` system call.
pub const SYS_CLOSE: u64 = 6;

/// The number of the `stat` system call.
pub const SYS_STAT: u64 = 7;

/// The number of the `exec` system call.
pub const SYS_EXEC: u64 = 8;

/// The number of the `meminfo` system call.
pub const SYS_MEMINFO: u64 = 9;

/// The number of the `procs` system call.
pub const SYS_PROCS: u64 = 10;

/// The number of the `shutdown` system call.
pub const SYS_SHUTDOWN: u64 = 11;

/// The number of the `reboot` system call.
pub const SYS_REBOOT: u64 = 12;

/// The number of the `format_number` system call.
pub const SYS_FORMAT_NUMBER: u64 = 13;

/// The number of the `wait` system call.
pub const SYS_WAIT: u64 = 14;

/// The number of the `get_pid` system call.
pub const SYS_GET_PID: u64 = 15;

/// The number of the `getuid` system call.
pub const SYS_GETUID: u64 = 17;

/// The number of the `getgid` system call.
pub const SYS_GETGID: u64 = 18;

/// `wait(pid)` - wait for process.
pub const SYS_WAIT: u64 = 14;

/// `get_pid()` - get current process id.
pub const SYS_GET_PID: u64 = 15;

/// `get_ppid()` - get parent process id.
pub const SYS_GET_PPID: u64 = 16;

/// The file descriptor of the console input.
pub const STDIN: u64 = 0;

/// The file descriptor of the console output.
pub const STDOUT: u64 = 1;

/// The file descriptor of the error output.
pub const STDERR: u64 = 2;

/// The first file descriptor a program gets for a file of the filesystem.
pub const FIRST_FILE_DESCRIPTOR: u64 = 3;

/// The longest output a single `write` call may produce.
///
/// The limit keeps a user program from flooding the console, and keeps the
/// kernel from copying an unbounded amount of user memory in one go.
pub const MAX_WRITE_LENGTH: u64 = 64 * 1024;

/// The longest text a program may hand to the kernel.
pub const MAX_TEXT_LENGTH: u64 = 1024;

/// The size of the metadata the `stat` call writes.
pub const METADATA_SIZE: u64 = 32;

/// The errors a system call reports.
pub mod error {
    /// The object does not exist.
    pub const NOT_FOUND: u64 = 1;
    /// The caller is not allowed to do what it asked for.
    pub const PERMISSION_DENIED: u64 = 2;
    /// The arguments of the call are not usable.
    pub const INVALID_ARGUMENT: u64 = 3;
    /// The medium failed.
    pub const IO: u64 = 4;
    /// The object exists already.
    pub const ALREADY_EXISTS: u64 = 5;
    /// The path does not name a directory.
    pub const NOT_A_DIRECTORY: u64 = 6;
    /// The directory is not empty.
    pub const NOT_EMPTY: u64 = 7;
    /// There is no free file handle left.
    pub const TOO_MANY_OPEN_FILES: u64 = 8;
    /// The filesystem is not mounted.
    pub const NOT_MOUNTED: u64 = 9;
    /// The operation is not supported.
    pub const UNSUPPORTED: u64 = 10;
    /// The kernel heap could not provide the memory the call needs.
    pub const NO_MEMORY: u64 = 11;
}

/// The value a system call returns when it fails.
///
/// The value is the two's complement of the number of the error, so the
/// program can tell a failure from a result by the sign of the number.
pub const fn failure(code: u64) -> u64 {
    u64::MAX - code + 1
}

/// Handle a system call.
///
/// The frame holds the register state of the interrupted user program. The
/// return value is written back into it, so the user program finds it in
/// `RAX` when the kernel returns to it.
pub fn handle(frame: &mut InterruptFrame) {
    let number = frame.rax;
    session::count_syscall();

    match number {
        SYS_WRITE => write(frame),
        SYS_EXIT => exit(frame),
        SYS_OPEN => open(frame),
        SYS_READ => read(frame),
        SYS_GETDENTS => getdents(frame),
        SYS_CLOSE => close(frame),
        SYS_STAT => stat(frame),
        SYS_EXEC => exec(frame),
        SYS_MEMINFO => meminfo(frame),
        SYS_PROCS => procs(frame),
        SYS_SHUTDOWN => shutdown(frame),
        SYS_REBOOT => reboot(frame),
        SYS_FORMAT_NUMBER => format_number(frame),
        SYS_WAIT => wait(frame),
        SYS_GET_PID => get_pid(frame),
        SYS_GET_PPID => get_ppid(frame),
        SYS_GETUID => getuid(frame),
        SYS_GETGID => getgid(frame),
        _ => unknown_syscall(number),
    }
}

/// `write(fd, buffer, length)`: write to the console or to a file.
///
/// Returns the number of bytes written.
fn write(frame: &mut InterruptFrame) {
    let fd = frame.rdi;
    let buffer = frame.rsi;
    let length = frame.rdx;

    if length > MAX_WRITE_LENGTH {
        frame.rax = failure(error::INVALID_ARGUMENT);
        return;
    }

    if fd == STDOUT || fd == STDERR {
        if let Err(error) = pointer::validate_access(buffer, length, false) {
            report_pointer("write", error);
            frame.rax = failure(error::INVALID_ARGUMENT);
            return;
        }
        // SAFETY: the pages of the buffer are mapped and accessible from
        // user mode, so the kernel may read them.
        let bytes = unsafe { core::slice::from_raw_parts(buffer as *const u8, length as usize) };
        Console::write(bytes);
        frame.rax = length;
        return;
    }

    let Some(handle) = handle_of(fd) else {
        frame.rax = failure(error::INVALID_ARGUMENT);
        return;
    };
    if let Err(error) = pointer::validate_access(buffer, length, false) {
        report_pointer("write", error);
        frame.rax = failure(error::INVALID_ARGUMENT);
        return;
    }
    // SAFETY: the pages of the buffer are mapped and accessible from user
    // mode, so the kernel may read them.
    let bytes = unsafe { core::slice::from_raw_parts(buffer as *const u8, length as usize) };
    let result = storage::with_filesystem(|fs| fs.write(handle, bytes));
    frame.rax = match result {
        Ok(Ok(count)) => count as u64,
        Ok(Err(error)) => failure(fs_error_code(error)),
        Err(_) => failure(error::NOT_MOUNTED),
    };
}

/// `exit(code)`: terminate the user program.
///
/// The function does not return: the kernel takes the program down and
/// resumes where it left user mode.
fn exit(frame: &mut InterruptFrame) -> ! {
    let code = frame.rdi;

    unsafe {
        serial::write_str("Elvaris kernel: user: exit: code=");
        serial::write_fmt(core::format_args!("{}", code));
        serial::write_newline();
    }

    session::terminate(session::OUTCOME_EXITED, code)
}

/// `open(path, length, flags)`: open a file of the filesystem.
///
/// Returns the number of the file descriptor.
fn open(frame: &mut InterruptFrame) {
    let path = match read_text(frame.rdi, frame.rsi) {
        Ok(path) => path,
        Err(code) => {
            frame.rax = failure(code);
            return;
        }
    };
    let flags = match open_flags(frame.rdx) {
        Ok(flags) => flags,
        Err(code) => {
            frame.rax = failure(code);
            return;
        }
    };

    let result = storage::with_filesystem(|fs| fs.open(&path, flags));
    frame.rax = match result {
        Ok(Ok(handle)) => handle.raw() as u64 + FIRST_FILE_DESCRIPTOR,
        Ok(Err(error)) => failure(fs_error_code(error)),
        Err(_) => failure(error::NOT_MOUNTED),
    };
}

/// `read(fd, buffer, length)`: read a line of the console or read a file.
///
/// Returns the number of bytes read.
fn read(frame: &mut InterruptFrame) {
    let fd = frame.rdi;
    let buffer = frame.rsi;
    let length = frame.rdx;

    if length == 0 || length > MAX_WRITE_LENGTH {
        frame.rax = failure(error::INVALID_ARGUMENT);
        return;
    }

    if fd == STDIN {
        if let Err(error) = pointer::validate_access(buffer, length, true) {
            report_pointer("read", error);
            frame.rax = failure(error::INVALID_ARGUMENT);
            return;
        }
        // SAFETY: the pages of the buffer are mapped and writable, so the
        // kernel may write the line into it.
        let bytes = unsafe { core::slice::from_raw_parts_mut(buffer as *mut u8, length as usize) };
        frame.rax = match Console::read_line(bytes) {
            Some(count) => count as u64,
            None => failure(error::IO),
        };
        return;
    }

    let Some(handle) = handle_of(fd) else {
        frame.rax = failure(error::INVALID_ARGUMENT);
        return;
    };
    if let Err(error) = pointer::validate_access(buffer, length, true) {
        report_pointer("read", error);
        frame.rax = failure(error::INVALID_ARGUMENT);
        return;
    }
    // SAFETY: the pages of the buffer are mapped and writable, so the
    // kernel may write the content of the file into it.
    let bytes = unsafe { core::slice::from_raw_parts_mut(buffer as *mut u8, length as usize) };
    let result = storage::with_filesystem(|fs| fs.read(handle, bytes));
    frame.rax = match result {
        Ok(Ok(count)) => count as u64,
        Ok(Err(error)) => failure(fs_error_code(error)),
        Err(_) => failure(error::NOT_MOUNTED),
    };
}

/// `getdents(path, length, buffer, length)`: list a directory.
///
/// The kernel writes the entries of the directory into the buffer of the
/// program. Every entry is a byte that says what the entry is, a byte that
/// holds the length of its name, and the name itself.
fn getdents(frame: &mut InterruptFrame) {
    let path = match read_text(frame.rdi, frame.rsi) {
        Ok(path) => path,
        Err(code) => {
            frame.rax = failure(code);
            return;
        }
    };
    let buffer = frame.rdx;
    let length = frame.rcx;

    if length == 0 || length > MAX_WRITE_LENGTH {
        frame.rax = failure(error::INVALID_ARGUMENT);
        return;
    }
    if let Err(error) = pointer::validate_access(buffer, length, true) {
        report_pointer("getdents", error);
        frame.rax = failure(error::INVALID_ARGUMENT);
        return;
    }

    let entries = match storage::with_filesystem(|fs| fs.read_dir(&path)) {
        Ok(Ok(entries)) => entries,
        Ok(Err(error)) => {
            frame.rax = failure(fs_error_code(error));
            return;
        }
        Err(_) => {
            frame.rax = failure(error::NOT_MOUNTED);
            return;
        }
    };

    let records = encode_entries(&entries);
    // SAFETY: the pages of the buffer are mapped and writable, and the
    // number of bytes that are written was checked against the length.
    let bytes = unsafe { core::slice::from_raw_parts_mut(buffer as *mut u8, length as usize) };
    let count = records.len().min(bytes.len());
    bytes[..count].copy_from_slice(&records[..count]);
    frame.rax = count as u64;
}

/// `close(fd)`: close a file of the filesystem.
fn close(frame: &mut InterruptFrame) {
    let fd = frame.rdi;
    let Some(handle) = handle_of(fd) else {
        frame.rax = failure(error::INVALID_ARGUMENT);
        return;
    };
    let result = storage::with_filesystem(|fs| fs.close(handle));
    frame.rax = match result {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => failure(fs_error_code(error)),
        Err(_) => failure(error::NOT_MOUNTED),
    };
}

/// `stat(path, length, buffer)`: read the metadata of an object.
///
/// The kernel writes the metadata into the buffer of the program: the kind
/// of the object, its size, its owner and its permissions.
fn stat(frame: &mut InterruptFrame) {
    let path = match read_text(frame.rdi, frame.rsi) {
        Ok(path) => path,
        Err(code) => {
            frame.rax = failure(code);
            return;
        }
    };
    let buffer = frame.rdx;

    if let Err(error) = pointer::validate_access(buffer, METADATA_SIZE, true) {
        report_pointer("stat", error);
        frame.rax = failure(error::INVALID_ARGUMENT);
        return;
    }

    let metadata = match storage::with_filesystem(|fs| fs.stat(&path)) {
        Ok(Ok(metadata)) => metadata,
        Ok(Err(error)) => {
            frame.rax = failure(fs_error_code(error));
            return;
        }
        Err(_) => {
            frame.rax = failure(error::NOT_MOUNTED);
            return;
        }
    };

    let kind = match metadata.file_type {
        FileType::File => 0u64,
        FileType::Directory => 1u64,
    };
    let owner = match metadata.owner {
        crate::fs::Owner::System => 0u64,
        crate::fs::Owner::User(uid) => uid + 1,
    };
    let mut permissions = 0u64;
    if metadata.permissions.read {
        permissions |= 1;
    }
    if metadata.permissions.write {
        permissions |= 2;
    }
    if metadata.permissions.execute {
        permissions |= 4;
    }

    // SAFETY: the pages of the buffer are mapped and writable, and the
    // metadata fits into the length that was checked.
    unsafe {
        let bytes = core::slice::from_raw_parts_mut(buffer as *mut u8, METADATA_SIZE as usize);
        bytes[0..8].copy_from_slice(&kind.to_le_bytes());
        bytes[8..16].copy_from_slice(&metadata.size.to_le_bytes());
        bytes[16..24].copy_from_slice(&owner.to_le_bytes());
        bytes[24..32].copy_from_slice(&permissions.to_le_bytes());
    }
    frame.rax = 0;
}

/// `exec(path, length, arguments, length)`: run a program of the filesystem.
///
/// The call returns the exit code of the program. The program runs before
/// the call returns, so the caller waits for it.
fn exec(frame: &mut InterruptFrame) {
    let path = match read_text(frame.rdi, frame.rsi) {
        Ok(path) => path,
        Err(code) => {
            frame.rax = failure(code);
            return;
        }
    };
    let arguments = match read_text(frame.rdx, frame.rcx) {
        Ok(arguments) => arguments,
        Err(code) => {
            frame.rax = failure(code);
            return;
        }
    };

    // The filesystem is released before the program runs: the program uses
    // the filesystem itself, and the lock of the filesystem is not
    // recursive.
    let prepared = match storage::with_filesystem(|fs| exec::prepare(fs, &path, &arguments)) {
        Ok(Ok(program)) => program,
        Ok(Err(error)) => {
            report_load_error("exec", error);
            frame.rax = failure(load_error_code(error));
            return;
        }
        Err(_) => {
            frame.rax = failure(error::NOT_MOUNTED);
            return;
        }
    };

    let parent = None;
    let pid = process::register(&path, parent, 0);
    process::set_running(pid);

    let result = exec::run(&prepared);
    match result {
        Ok(result) => {
            process::finish(pid, exit_status(result.outcome));
            let _ = crate::process::wait(Some(pid));
            crate::process::cleanup_process(pid);
            frame.rax = exec::exit_code(result.outcome);
        }
        Err(error) => {
            process::finish(pid, ExitStatus::Pending);
            let _ = crate::process::wait(Some(pid));
            crate::process::cleanup_process(pid);
            report_load_error("exec", error);
            frame.rax = failure(load_error_code(error));
        }
    }
}

/// `meminfo()`: report the memory of the machine.
fn meminfo(frame: &mut InterruptFrame) {
    sysinfo::report_memory();
    frame.rax = 0;
}

/// `procs()`: report the processes of the machine.
fn procs(frame: &mut InterruptFrame) {
    process::report();
    frame.rax = 0;
}

/// `wait(pid)`: wait for a process.
fn wait(frame: &mut InterruptFrame) {
    let pid_arg = frame.rdi;
    let pid = if pid_arg == u64::MAX || pid_arg == 0 { None } else { Some(crate::process::ProcessId(pid_arg)) };
    match crate::process::wait(pid) {
        Some((_pid, status)) => {
            let code = match status {
                ExitStatus::Exited(c) => c,
                _ => crate::user::exec::ABNORMAL_EXIT_CODE,
            };
            frame.rax = code;
        }
        None => {
            frame.rax = failure(error::NOT_FOUND);
        }
    }
}

/// `get_pid()`: get current process id.
fn get_pid(frame: &mut InterruptFrame) {
    frame.rax = 1;
}

/// `get_ppid()`: get parent process id.
fn get_ppid(frame: &mut InterruptFrame) {
    frame.rax = 0;
}

/// `shutdown()`: stop the machine.
fn shutdown(frame: &mut InterruptFrame) {
    unsafe {
        serial::write_str("Elvaris kernel: shutdown: the machine stops\r\n");
    }
    // The files of the machine are written back before it stops, so they
    // survive a restart.
    let _ = crate::storage::sync();
    // SAFETY: the machine stops here, and the code tells the host that the
    // boot was successful.
    unsafe { crate::exit::exit(crate::exit::EXIT_SUCCESS) };
    frame.rax = 0;
}

fn wait(frame: &mut InterruptFrame) {
    let pid_arg = frame.rdi;
    let pid = if pid_arg == u64::MAX || pid_arg == 0 { None } else { Some(ProcessId(pid_arg)) };
    match process::wait(pid) {
        Some((_pid, status)) => {
            let code = match status {
                ExitStatus::Exited(c) => c,
                ExitStatus::Faulted(_) => crate::user::exec::ABNORMAL_EXIT_CODE,
                ExitStatus::Rejected(_) => crate::user::exec::ABNORMAL_EXIT_CODE,
                ExitStatus::Pending => crate::user::exec::ABNORMAL_EXIT_CODE,
            };
            frame.rax = code;
        }
        None => {
            frame.rax = failure(error::NOT_FOUND);
        }
    }
}

/// `getuid()`: get current user id.
fn getuid(frame: &mut InterruptFrame) {
    frame.rax = 0; // root for now
}

/// `getgid()`: get current group id.
fn getgid(frame: &mut InterruptFrame) {
    frame.rax = 0;
}

/// `shutdown()`: stop the machine.
fn shutdown(frame: &mut InterruptFrame) {
    unsafe {
        serial::write_str("Elvaris kernel: reboot: the machine cannot reset itself yet\r\n");
    }
    // The files of the machine are written back before it stops, so they
    // survive a restart.
    let _ = crate::storage::sync();
    unsafe { crate::exit::exit(crate::exit::EXIT_SUCCESS) };
    frame.rax = 0;
}

/// `format_number(value, buffer, length)`: render a number.
///
/// The kernel writes the decimal representation of the number into the buffer
/// of the program and returns the number of bytes it wrote.
fn format_number(frame: &mut InterruptFrame) {
    let value = frame.rdi;
    let buffer = frame.rsi;
    let length = frame.rdx;

    if length == 0 || length > MAX_TEXT_LENGTH {
        frame.rax = failure(error::INVALID_ARGUMENT);
        return;
    }
    if let Err(error) = pointer::validate_access(buffer, length, true) {
        report_pointer("format_number", error);
        frame.rax = failure(error::INVALID_ARGUMENT);
        return;
    }

    let mut digits = [0u8; 24];
    let count = render_number(value, &mut digits);
    // SAFETY: the pages of the buffer are mapped and writable, and the
    // number of bytes that are written was checked against the length.
    let bytes = unsafe { core::slice::from_raw_parts_mut(buffer as *mut u8, length as usize) };
    let count = count.min(bytes.len());
    bytes[..count].copy_from_slice(&digits[..count]);
    frame.rax = count as u64;
}

/// Reject a system call the kernel does not know.
fn unknown_syscall(number: u64) -> ! {
    unsafe {
        serial::write_str("Elvaris kernel: user: invalid system call: number=");
        serial::write_fmt(core::format_args!("{}", number));
        serial::write_newline();
    }

    session::terminate(session::OUTCOME_REJECTED, number)
}

/// Reject a system call whose arguments the kernel cannot use.
fn invalid_argument(call: &str, reason: &str) -> ! {
    unsafe {
        serial::write_str("Elvaris kernel: user: invalid argument: ");
        serial::write_str(call);
        serial::write_str(": ");
        serial::write_str(reason);
        serial::write_newline();
    }

    session::terminate(session::OUTCOME_REJECTED, 0)
}

/// Reject a system call whose pointer does not belong to the user program.
fn report_pointer(call: &str, error: pointer::PointerError) {
    unsafe {
        serial::write_str("Elvaris kernel: user: invalid pointer: ");
        serial::write_str(call);
        serial::write_str(": ");
        serial::write_str(error.description());
        serial::write_newline();
    }
}

/// Report a program that could not be loaded.
fn report_load_error(call: &str, error: LoadError) {
    unsafe {
        serial::write_str("Elvaris kernel: user: ");
        serial::write_str(call);
        serial::write_str(": the program could not be loaded: ");
        serial::write_str(load_error_description(error));
        serial::write_newline();
    }
}

/// The handle of the filesystem a file descriptor stands for.
fn handle_of(fd: u64) -> Option<FileHandle> {
    if fd < FIRST_FILE_DESCRIPTOR {
        return None;
    }
    let index = fd - FIRST_FILE_DESCRIPTOR;
    if index > u32::MAX as u64 {
        return None;
    }
    Some(FileHandle(index as u32))
}

/// The flags a program may ask for.
fn open_flags(flags: u64) -> Result<OpenFlags, u64> {
    let read = flags & 0x1 != 0;
    let write = flags & 0x2 != 0;
    let create = flags & 0x4 != 0;
    let truncate = flags & 0x8 != 0;
    if flags & !0xF != 0 {
        return Err(error::INVALID_ARGUMENT);
    }
    if !read && !write {
        return Err(error::INVALID_ARGUMENT);
    }
    Ok(OpenFlags {
        read,
        write,
        create,
        truncate,
        execute: false,
    })
}

/// Read a text a program handed to the kernel.
fn read_text(address: u64, length: u64) -> Result<String, u64> {
    if length == 0 {
        return Ok(String::new());
    }
    if length > MAX_TEXT_LENGTH {
        return Err(error::INVALID_ARGUMENT);
    }
    if pointer::validate_access(address, length, false).is_err() {
        return Err(error::INVALID_ARGUMENT);
    }
    // SAFETY: the range is mapped and accessible from user mode, so the
    // kernel may read it.
    let bytes = unsafe { core::slice::from_raw_parts(address as *const u8, length as usize) };
    String::from_utf8(bytes.to_vec()).map_err(|_| error::INVALID_ARGUMENT)
}

/// The entries of a directory, in the form the `getdents` call writes them.
fn encode_entries(entries: &[crate::fs::DirEntry]) -> Vec<u8> {
    let mut records = Vec::new();
    for entry in entries {
        let name = entry.name.as_bytes();
        let length = name.len().min(255);
        records.push(match entry.file_type {
            FileType::Directory => b'd',
            FileType::File => b'f',
        });
        records.push(length as u8);
        records.extend_from_slice(&name[..length]);
    }
    records
}

/// The decimal representation of a number.
fn render_number(value: u64, buffer: &mut [u8]) -> usize {
    let mut digits = [0u8; 20];
    let mut count = 0;
    let mut rest = value;
    if rest == 0 {
        digits[0] = b'0';
        count = 1;
    } else {
        while rest > 0 {
            digits[count] = b'0' + (rest % 10) as u8;
            rest /= 10;
            count += 1;
        }
    }
    let count = count.min(buffer.len());
    for index in 0..count {
        buffer[index] = digits[count - 1 - index];
    }
    count
}

/// The exit status of a program that ended.
fn exit_status(outcome: session::Outcome) -> ExitStatus {
    match outcome {
        session::Outcome::Exited(code) => ExitStatus::Exited(code),
        session::Outcome::Faulted(vector) => {
            ExitStatus::Faulted(crate::process::process::FaultInfo {
                vector,
                error_code: 0,
                fault_address: 0,
                rip: 0,
                rsp: 0,
            })
        }
        session::Outcome::Rejected(reason) => {
            ExitStatus::Rejected(crate::process::process::RejectInfo { reason })
        }
        session::Outcome::Running => ExitStatus::Pending,
    }
}

/// The number of the error the filesystem reports.
fn fs_error_code(error: FsError) -> u64 {
    match error {
        FsError::NotFound => error::NOT_FOUND,
        FsError::InvalidPath => error::INVALID_ARGUMENT,
        FsError::PermissionDenied => error::PERMISSION_DENIED,
        FsError::IoError => error::IO,
        FsError::AlreadyExists => error::ALREADY_EXISTS,
        FsError::NotADirectory => error::NOT_A_DIRECTORY,
        FsError::IsADirectory => error::INVALID_ARGUMENT,
        FsError::NotEmpty => error::NOT_EMPTY,
        FsError::TooManyOpenFiles => error::TOO_MANY_OPEN_FILES,
        FsError::InvalidArgument => error::INVALID_ARGUMENT,
        FsError::NotMounted => error::NOT_MOUNTED,
        FsError::Unsupported => error::UNSUPPORTED,
    }
}

/// The number of the error the loader reports.
fn load_error_code(error: LoadError) -> u64 {
    match error {
        LoadError::NotFound => error::NOT_FOUND,
        LoadError::NotAFile => error::INVALID_ARGUMENT,
        LoadError::PermissionDenied => error::PERMISSION_DENIED,
        LoadError::IoError => error::IO,
        LoadError::InvalidImage(_) => error::INVALID_ARGUMENT,
        LoadError::ProgramTooLarge => error::INVALID_ARGUMENT,
        LoadError::ArgumentsTooLarge => error::INVALID_ARGUMENT,
        LoadError::OutOfMemory => error::NO_MEMORY,
        LoadError::Mapping(_) => error::IO,
        LoadError::InvalidPath => error::INVALID_ARGUMENT,
    }
}

/// A short description of the error the loader reports.
fn load_error_description(error: LoadError) -> &'static str {
    exec::load_error_description(error)
}

/// The reason a system call was refused, for the diagnostics.
pub fn describe_failure(value: u64) -> &'static str {
    match value {
        error::NOT_FOUND => "the object does not exist",
        error::PERMISSION_DENIED => "the operation is not permitted",
        error::INVALID_ARGUMENT => "the arguments are not valid",
        error::IO => "the medium failed",
        error::ALREADY_EXISTS => "the object exists already",
        error::NOT_A_DIRECTORY => "the path does not name a directory",
        error::NOT_EMPTY => "the directory is not empty",
        error::TOO_MANY_OPEN_FILES => "there is no free file handle left",
        error::NOT_MOUNTED => "the filesystem is not mounted",
        error::UNSUPPORTED => "the operation is not supported",
        error::NO_MEMORY => "the kernel heap is full",
        _ => "the call failed",
    }
}

/// The call that made the kernel refuse a system call.
///
/// The function is the counterpart of [`handle`] for the tests: it reports
/// the call that was refused.
pub fn refused_call(call: &str) -> ! {
    invalid_argument(call, "the call was refused")
}
