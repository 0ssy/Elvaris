//! The system-call ABI.
//!
//! A user program reaches the kernel through one instruction: `int 0x80`. The
//! interrupt descriptor table entry of that vector is the only entry the
//! processor accepts from user mode, so a user program cannot raise any
//! other interrupt.
//!
//! The registers follow the System V convention:
//!
//! | Register | Meaning                                               |
//! |----------|-------------------------------------------------------|
//! | `RAX`    | the number of the system call                          |
//! | `RDI`    | the first argument                                    |
//! | `RSI`    | the second argument                                   |
//! | `RDX`    | the third argument                                    |
//! | `RAX`    | the return value                                      |
//!
//! The arguments are untrusted. Every pointer a program passes is validated
//! against the user address space and against the page tables before the
//! kernel dereferences it.

use crate::cpu::vectors::InterruptFrame;
use crate::serial;
use crate::user::pointer;
use crate::user::session;

/// The number of the `write` system call.
pub const SYS_WRITE: u64 = 1;

/// The number of the `exit` system call.
pub const SYS_EXIT: u64 = 2;

/// The file descriptor of the serial console.
pub const STDOUT: u64 = 1;

/// The longest output a single `write` call may produce.
///
/// The limit keeps a user program from flooding the console, and keeps the
/// kernel from copying an unbounded amount of user memory in one go.
pub const MAX_WRITE_LENGTH: u64 = 64 * 1024;

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
        _ => unknown_syscall(number),
    }
}

/// `write(fd, buffer, length)`: write to the serial console.
///
/// Returns the number of bytes written.
fn write(frame: &mut InterruptFrame) {
    let fd = frame.rdi;
    let buffer = frame.rsi;
    let length = frame.rdx;

    if fd != STDOUT {
        invalid_argument("write", "the file descriptor is not the console");
    }
    if length > MAX_WRITE_LENGTH {
        invalid_argument("write", "the length is longer than the kernel accepts");
    }

    if let Err(error) = pointer::validate_access(buffer, length, false) {
        invalid_pointer("write", error);
    }

    // The pages of the buffer are mapped and accessible from user mode, so
    // the kernel may read them.
    let bytes = unsafe { core::slice::from_raw_parts(buffer as *const u8, length as usize) };
    unsafe { serial::write_bytes(bytes) };

    frame.rax = length;
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
fn invalid_pointer(call: &str, error: pointer::PointerError) -> ! {
    unsafe {
        serial::write_str("Elvaris kernel: user: invalid pointer: ");
        serial::write_str(call);
        serial::write_str(": ");
        serial::write_str(error.description());
        serial::write_newline();
    }

    session::terminate(session::OUTCOME_REJECTED, 0)
}
