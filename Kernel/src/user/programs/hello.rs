//! The program that greets the person at the console.
//!
//! The program is the first external program of the shell: it writes a line
//! to the console and ends with the code zero. It is the smallest program
//! that shows how a program of the filesystem runs: the kernel loads it, it
//! asks the kernel for the console, and it ends.
//!
//! # The entry convention
//!
//! The kernel enters the program with the number of the arguments in `RDI`
//! and the table of the arguments in `RSI`. The program needs neither, but
//! it must not rely on the content of the other registers.

use core::arch::global_asm;

global_asm!(
    ".section .user_hello_text,\"ax\"\n",
    ".globl elvaris_hello_program\n",
    "elvaris_hello_program:\n",
    // write(1, message, length)
    "    mov eax, 1\n",
    "    mov edi, 1\n",
    "    lea rsi, [rip + .Lhello_message]\n",
    "    lea rdx, [rip + .Lhello_message_end]\n",
    "    sub rdx, rsi\n",
    "    int 0x80\n",
    // exit(0)
    "    mov eax, 2\n",
    "    xor edi, edi\n",
    "    int 0x80\n",
    // The kernel never returns from `exit`. Reaching this instruction means
    // the system call path is broken, and the fault it raises ends the
    // program instead of the kernel.
    "    ud2\n",
    ".Lhello_message:\n",
    "    .ascii \"Hello from the Elvaris userland!\\n\"\n",
    ".Lhello_message_end:\n",
    ".globl elvaris_hello_text_end\n",
    "elvaris_hello_text_end:\n",
);

unsafe extern "C" {
    static elvaris_hello_program: u8;
    static elvaris_hello_text_end: u8;
}

/// The machine code of the program.
///
/// The bytes are the program itself; the kernel copies them into the user
/// code page before it enters user mode.
pub fn payload() -> &'static [u8] {
    // SAFETY: the two symbols are labels of the same assembly block, so the
    // range between them is the program. The program is never modified.
    unsafe {
        let start = core::ptr::addr_of!(elvaris_hello_program) as usize;
        let end = core::ptr::addr_of!(elvaris_hello_text_end) as usize;
        core::slice::from_raw_parts(start as *const u8, end - start)
    }
}
