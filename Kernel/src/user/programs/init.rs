//! The first program of the machine.
//!
//! The kernel starts this program: it is the program that brings the shell
//! up. The program is deliberately small, because it has only one job.
//!
//! # What the program does
//!
//! 1. It starts the shell with the `exec` system call.
//! 2. It waits for the shell to end: the call returns when the shell ended.
//! 3. It ends with the code the shell ended with, so that the state of the
//!    shell reaches the kernel.
//!
//! When the shell cannot be started at all, the program says so on the
//! console and ends with the code one. A machine whose shell does not run is
//! a machine that says why.

use core::arch::global_asm;

global_asm!(
    ".section .user_init_text,\"ax\"\n",
    ".globl elvaris_init_program\n",
    "elvaris_init_program:\n",
    // exec("/bin/sh", "")
    "    mov eax, 8\n",
    "    lea rdi, [rip + .Linit_path]\n",
    "    lea rsi, [rip + .Linit_path_end]\n",
    "    sub rsi, rdi\n",
    "    xor edx, edx\n",
    "    xor ecx, ecx\n",
    "    int 0x80\n",
    // The kernel reports a failure as a negative number. The program says so
    // and ends with the code one.
    "    cmp rax, 0\n",
    "    jl .Linit_failed\n",
    // exit(the code of the shell)
    "    mov edi, eax\n",
    "    mov eax, 2\n",
    "    int 0x80\n",
    "    ud2\n",
    ".Linit_failed:\n",
    "    mov eax, 1\n",
    "    mov edi, 1\n",
    "    lea rsi, [rip + .Linit_message]\n",
    "    lea rdx, [rip + .Linit_message_end]\n",
    "    sub rdx, rsi\n",
    "    int 0x80\n",
    "    mov eax, 2\n",
    "    mov edi, 1\n",
    "    int 0x80\n",
    "    ud2\n",
    ".Linit_path:\n",
    "    .ascii \"/bin/sh\"\n",
    ".Linit_path_end:\n",
    ".Linit_message:\n",
    "    .ascii \"init: the shell could not be started\\n\"\n",
    ".Linit_message_end:\n",
    ".globl elvaris_init_text_end\n",
    "elvaris_init_text_end:\n",
);

unsafe extern "C" {
    static elvaris_init_program: u8;
    static elvaris_init_text_end: u8;
}

/// The machine code of the program.
pub fn payload() -> &'static [u8] {
    // SAFETY: the two symbols are labels of the same assembly block, so the
    // range between them is the program. The program is never modified.
    unsafe {
        let start = core::ptr::addr_of!(elvaris_init_program) as usize;
        let end = core::ptr::addr_of!(elvaris_init_text_end) as usize;
        core::slice::from_raw_parts(start as *const u8, end - start)
    }
}
