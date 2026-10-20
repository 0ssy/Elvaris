//! The user session: the kernel-owned state of the single user program.
//!
//! # The privilege transition
//!
//! The kernel enters user mode with `iretq`. The instruction pops the
//! instruction pointer, the code segment, the flags, the stack pointer and
//! the stack segment of the interrupted code, so the frame the kernel builds
//! on its own stack is exactly the frame `iretq` expects:
//!
//! ```text
//!  high  ┌───────────────┐
//!        │ ss            │  user stack segment
//!        │ rsp           │  user stack pointer
//!        │ rflags        │  user flags
//!        │ cs            │  user code segment
//!  low   │ rip           │  first instruction of the user program
//!        └───────────────┘
//! ```
//!
//! The kernel also loads the user data segment into `DS` and `ES` before
//! the transition, so that the user program does not have to do it before
//! it touches memory.
//!
//! # The way back
//!
//! The user program ends in one of two ways: it calls `exit`, or it causes
//! an exception. Both are handled in an interrupt context, on the stack the
//! task state segment reserves for events that arrive from user mode.
//!
//! Returning to the kernel from there is a matter of restoring the kernel
//! stack pointer and jumping to the address the kernel saved when it entered
//! user mode. The assembly stub that performs the transition stores that
//! stack pointer and that address in the session, so the kernel does not
//! have to guess them.

use core::arch::global_asm;

use crate::cpu::gdt::{USER_CODE_SELECTOR, USER_DATA_SELECTOR};
use crate::cpu::vectors::InterruptFrame;
use crate::serial;

global_asm!(
    ".section .text\n",
    ".globl elvaris_run_user\n",
    "elvaris_run_user:\n",
    // The System V ABI makes these registers callee-saved, so they have to
    // survive the transition.
    "    push rbp\n",
    "    mov rbp, rsp\n",
    "    push rbx\n",
    "    push r12\n",
    "    push r13\n",
    "    push r14\n",
    "    push r15\n",
    // Arguments: rdi = session, rsi = rip, rdx = rsp, rcx = cs, r8 = ss,
    // r9 = rflags.
    //
    // Remember where the kernel resumes: the stack pointer of this frame and
    // the address of the resume label.
    "    mov [rdi], rsp\n",
    "    lea rax, [rip + elvaris_user_resume]\n",
    "    mov [rdi+8], rax\n",
    // The arguments of the user program live in the session: the message
    // pointer and the length of the message. The kernel hands them over in
    // RDI and RSI, the way the entry convention of the program describes.
    "    mov r10, [rdi+16]\n",
    "    mov r11, [rdi+24]\n",
    // Build the frame that iretq pops, from the bottom up.
    "    sub rsp, 40\n",
    "    mov [rsp], rsi\n",
    "    mov [rsp+8], rcx\n",
    "    mov [rsp+16], r9\n",
    "    mov [rsp+24], rdx\n",
    "    mov [rsp+32], r8\n",
    // The user program starts with the arguments of its entry convention.
    "    mov rdi, r10\n",
    "    mov rsi, r11\n",
    // The user program expects the user data segment in DS and ES.
    "    mov ax, 0x23\n",
    "    mov ds, ax\n",
    "    mov es, ax\n",
    "    iretq\n",
    ".globl elvaris_user_resume\n",
    "elvaris_user_resume:\n",
    // The kernel owns the segment registers again.
    "    mov ax, 0x10\n",
    "    mov ds, ax\n",
    "    mov es, ax\n",
    // Drop the frame of the transition. The stack pointer is the one the
    // kernel saved before it entered user mode.
    "    pop r15\n",
    "    pop r14\n",
    "    pop r13\n",
    "    pop r12\n",
    "    pop rbx\n",
    "    pop rbp\n",
    "    ret\n",
);

unsafe extern "C" {
    /// Enter user mode. The function returns when the user program ended and
    /// the kernel resumed.
    fn elvaris_run_user(
        session: *mut UserSession,
        rip: u64,
        rsp: u64,
        cs: u64,
        ss: u64,
        rflags: u64,
    );
}

/// The privilege level of the user segments.
pub const USER_RPL: u16 = 3;

/// The flags the user program starts with.
///
/// The interrupt flag is set: a user program is allowed to be interrupted,
/// and the timer interrupt is handled on the stack the task state segment
/// reserves for it. Bit one is reserved and always reads as one.
pub const USER_RFLAGS: u64 = 0x202;

/// The user program is still running.
pub const OUTCOME_RUNNING: u64 = 0;

/// The user program called `exit`.
pub const OUTCOME_EXITED: u64 = 1;

/// The user program caused an exception.
pub const OUTCOME_FAULTED: u64 = 2;

/// The kernel refused a system call or one of its arguments.
pub const OUTCOME_REJECTED: u64 = 3;

/// The state of the single user program.
///
/// The kernel owns the program for its whole lifetime: it creates it, enters
/// it, and takes it down again when it ends.
///
/// The first four fields have a fixed layout: the assembly stub that enters
/// user mode writes the resume stack pointer and the resume address into the
/// first two of them, and reads the arguments of the user program out of the
/// next two.
#[repr(C)]
pub struct UserSession {
    /// The kernel stack pointer the kernel resumes with.
    ///
    /// The assembly stub writes it when the user mode is entered.
    pub resume_rsp: u64,
    /// The address the kernel resumes at.
    ///
    /// The assembly stub writes it when the user mode is entered.
    pub resume_rip: u64,
    /// The pointer to the message the user program is expected to write.
    ///
    /// The assembly stub hands it to the program in `RDI`.
    pub message_pointer: u64,
    /// The length of that message.
    ///
    /// The assembly stub hands it to the program in `RSI`.
    pub message_length: u64,
    /// How the user program ended: one of the `OUTCOME_` constants.
    pub outcome: u64,
    /// The exit code of the user program, or the vector of the exception it
    /// caused.
    pub detail: u64,
    /// The number of system calls the user program made.
    pub syscalls: u64,
}

impl UserSession {
    const fn new() -> Self {
        Self {
            resume_rsp: 0,
            resume_rip: 0,
            message_pointer: 0,
            message_length: 0,
            outcome: OUTCOME_RUNNING,
            detail: 0,
            syscalls: 0,
        }
    }
}

/// The session of the single user program.
static mut SESSION: UserSession = UserSession::new();

/// How the user program ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The program called `exit` with the given code.
    Exited(u64),
    /// The program caused the given CPU exception.
    Faulted(u8),
    /// The kernel refused a system call of the program.
    Rejected(u64),
    /// The program is still running. The kernel never observes this state.
    Running,
}

/// The session of the single user program.
fn session() -> &'static mut UserSession {
    // SAFETY: the kernel owns exactly one user program, and the session is
    // only used from the kernel context that owns it.
    unsafe { &mut *core::ptr::addr_of_mut!(SESSION) }
}

/// Whether a user program is running.
///
/// The kernel uses this to tell an exception caused by the user program from
/// an exception that arrived from user mode without a program to attribute
/// it to.
pub fn is_active() -> bool {
    let session = session();
    session.resume_rsp != 0
}

/// Run the user program and return when it ends.
///
/// The program starts at `entry` with `stack_top` as its stack pointer. The
/// pointer and the length of the message the program is expected to write are
/// handed over in `RDI` and `RSI`, the way the entry convention of the
/// program describes.
pub fn run(entry: u64, stack_top: u64, message_pointer: u64, message_length: u64) -> Outcome {
    {
        let session = session();
        session.outcome = OUTCOME_RUNNING;
        session.detail = 0;
        session.syscalls = 0;
        session.message_pointer = message_pointer;
        session.message_length = message_length;
    }

    // SAFETY: the session outlives the call, and the frame the assembly stub
    // builds describes the user state the kernel wants to enter.
    unsafe {
        elvaris_run_user(
            core::ptr::addr_of_mut!(SESSION),
            entry,
            stack_top,
            (USER_CODE_SELECTOR | USER_RPL) as u64,
            (USER_DATA_SELECTOR | USER_RPL) as u64,
            USER_RFLAGS,
        );
    }

    let outcome = {
        let session = session();
        let outcome = match session.outcome {
            OUTCOME_EXITED => Outcome::Exited(session.detail),
            OUTCOME_FAULTED => Outcome::Faulted(session.detail as u8),
            OUTCOME_REJECTED => Outcome::Rejected(session.detail),
            _ => Outcome::Running,
        };
        // The user program is gone: nothing may resume it.
        session.resume_rsp = 0;
        session.resume_rip = 0;
        outcome
    };

    outcome
}

/// Terminate the user program and return to the kernel.
///
/// The function does not return to the interrupted user program: it restores
/// the kernel stack and resumes the kernel where it left user mode.
pub fn terminate(outcome: u64, detail: u64) -> ! {
    {
        let session = session();
        session.outcome = outcome;
        session.detail = detail;
    }
    return_to_kernel()
}

/// Restore the kernel stack and resume the kernel.
fn return_to_kernel() -> ! {
    let (stack_pointer, resume) = {
        let session = session();
        (session.resume_rsp, session.resume_rip)
    };

    // SAFETY: the stack pointer and the resume address were written by the
    // assembly stub when the kernel entered user mode. The jump leaves the
    // kernel stack of the interrupted user program for good.
    unsafe {
        core::arch::asm!(
            "mov rsp, {0}\n\tjmp {1}",
            in(reg) stack_pointer,
            in(reg) resume,
            options(noreturn),
        );
    }
}

/// Count a system call the user program made.
pub fn count_syscall() {
    let session = session();
    session.syscalls += 1;
}

/// The number of system calls the user program made.
pub fn syscall_count() -> u64 {
    let session = session();
    session.syscalls
}

/// Handle an exception the user program caused.
///
/// The exception does not take the kernel down. The user program is
/// terminated, the reason is reported on the serial console, and the kernel
/// resumes where it left user mode.
pub fn handle_fault(frame: &InterruptFrame, vector: u8) -> ! {
    unsafe {
        serial::write_str("Elvaris kernel: user: fault in user program\r\n");
        serial::write_str("Elvaris kernel: user: fault: vector=");
        serial::write_fmt(core::format_args!("{}", vector));
        if let Some(name) = crate::cpu::vectors::exception_name(vector) {
            serial::write_str(" name=");
            serial::write_str(name);
        }
        serial::write_newline();
        serial::write_str("Elvaris kernel: user: fault: error_code=0x");
        serial::write_fmt(core::format_args!("{:x}", frame.error_code()));
        serial::write_str(" rip=0x");
        serial::write_fmt(core::format_args!("{:x}", frame.rip()));
        serial::write_str(" rsp=0x");
        serial::write_fmt(core::format_args!("{:x}", frame.rsp()));
        serial::write_newline();
        if vector == crate::cpu::vectors::vector::PAGE_FAULT {
            serial::write_str("Elvaris kernel: user: fault: address=0x");
            serial::write_fmt(core::format_args!("{:x}", crate::cpu::exception::read_cr2()));
            serial::write_newline();
        }
    }

    terminate(OUTCOME_FAULTED, vector as u64)
}
