//! The user program.
//!
//! The user program is a fixed image of machine code that the kernel
//! assembles itself. There is no executable format yet: the image is a block
//! of bytes that the kernel copies into the user code page, and the entry
//! convention is fixed.
//!
//! # Entry convention
//!
//! The kernel enters the program with
//!
//! - `RDI` pointing at the message the program is expected to write,
//! - `RSI` holding the length of that message,
//! - `DX` holding zero.
//!
//! The registers are the ones the processor happens to leave behind, so the
//! kernel sets them explicitly before the transition.
//!
//! # The probes
//!
//! The image holds three programs. The first one is the user program proper.
//! The other two are probes: they do something the kernel has to refuse, and
//! the kernel takes them down instead of letting them damage the machine.
//! The kernel runs them as a self test, so that the behaviour of the
//! privilege boundary is proven on every boot and not only described.

use core::arch::global_asm;

use crate::user::address::{USER_ADDRESS_SPACE_END, USER_CODE_BASE};

/// The system call number the unknown-system-call probe uses.
///
/// The kernel knows the numbers one and two; every other number is refused.
const UNKNOWN_SYSCALL_NUMBER: u64 = 99;

global_asm!(
    ".section .user_text,\"ax\"\n",
    ".globl elvaris_user_program\n",
    "elvaris_user_program:\n",
    // RDI = message pointer, RSI = message length. Keep them: the system
    // call convention uses the same registers for other purposes.
    "    mov r8, rdi\n",
    "    mov r9, rsi\n",
    // write(1, message, length)
    "    mov eax, 1\n",
    "    mov edi, 1\n",
    "    mov rsi, r8\n",
    "    mov rdx, r9\n",
    "    int 0x80\n",
    // exit(0)
    "    mov eax, 2\n",
    "    xor edi, edi\n",
    "    int 0x80\n",
    // The kernel never returns from `exit`. Reaching this instruction means
    // the system call path is broken, and the fault it raises terminates the
    // user program instead of the kernel.
    "    ud2\n",
    // The fault probe: it reads from the first address above the user
    // address space, which is not mapped. The processor raises a page fault,
    // and the kernel takes the program down instead of taking the machine
    // down with it.
    ".globl elvaris_user_fault_probe\n",
    "elvaris_user_fault_probe:\n",
    "    mov rax, {fault_address}\n",
    "    mov rbx, [rax]\n",
    // The kernel never returns from a fault either.
    "    ud2\n",
    // The unknown-system-call probe: it raises a system call the kernel does
    // not know. The kernel refuses it and takes the program down.
    ".globl elvaris_user_unknown_syscall_probe\n",
    "elvaris_user_unknown_syscall_probe:\n",
    "    mov eax, {syscall_number}\n",
    "    int 0x80\n",
    "    ud2\n",
    ".globl elvaris_user_text_end\n",
    "elvaris_user_text_end:\n",
    fault_address = const USER_ADDRESS_SPACE_END,
    syscall_number = const UNKNOWN_SYSCALL_NUMBER,
);

unsafe extern "C" {
    static elvaris_user_program: u8;
    static elvaris_user_fault_probe: u8;
    static elvaris_user_unknown_syscall_probe: u8;
    static elvaris_user_text_end: u8;
}

/// The message the user program writes to the console.
pub const MESSAGE: &[u8] = b"Hello from user space!\r\n";

/// The machine code of the user program.
///
/// The bytes are the programs themselves; the kernel copies them into the
/// user code page before it enters user mode.
pub fn image() -> &'static [u8] {
    // SAFETY: the two symbols are labels of the same assembly block, so the
    // range between them is the image. The image is never modified.
    unsafe {
        let start = core::ptr::addr_of!(elvaris_user_program) as usize;
        let end = core::ptr::addr_of!(elvaris_user_text_end) as usize;
        core::slice::from_raw_parts(start as *const u8, end - start)
    }
}

/// The offset of a label inside the image.
fn offset_of(symbol: *const u8) -> u64 {
    let start = core::ptr::addr_of!(elvaris_user_program) as usize;
    (symbol as usize - start) as u64
}

/// The address the user program starts at.
///
/// The program is loaded at the base of the user code page, so the entry
/// address is the base of that page.
pub const fn entry_address() -> u64 {
    USER_CODE_BASE
}

/// The address the fault probe starts at.
///
/// The probe follows the user program in the same code page.
pub fn fault_probe_address() -> u64 {
    USER_CODE_BASE + offset_of(core::ptr::addr_of!(elvaris_user_fault_probe))
}

/// The address the unknown-system-call probe starts at.
///
/// The probe follows the user program in the same code page.
pub fn unknown_syscall_probe_address() -> u64 {
    USER_CODE_BASE + offset_of(core::ptr::addr_of!(
        elvaris_user_unknown_syscall_probe
    ))
}

/// The system call number the unknown-system-call probe uses.
pub const fn unknown_syscall_number() -> u64 {
    UNKNOWN_SYSCALL_NUMBER
}
