//! Context switching implementation for x86_64.

use crate::threads::context::ThreadContext;
use core::arch::global_asm;

global_asm!(
    ".section .text\n",
    ".globl switch_context\n",
    "switch_context:\n",
    // Save current context (callee-saved registers)
    // RDI = current context, RSI = next context
    "    mov [rdi], r15\n",
    "    mov [rdi+8], r14\n",
    "    mov [rdi+16], r13\n",
    "    mov [rdi+24], r12\n",
    "    mov [rdi+32], rbp\n",
    "    mov [rdi+40], rbx\n",
    // Save RIP (return address)
    "    mov rax, [rsp]\n",
    "    mov [rdi+48], rax\n",
    // Save RSP
    "    lea rax, [rsp+8]\n",
    "    mov [rdi+56], rax\n",
    // Restore next context
    "    mov r15, [rsi]\n",
    "    mov r14, [rsi+8]\n",
    "    mov r13, [rsi+16]\n",
    "    mov r12, [rsi+24]\n",
    "    mov rbp, [rsi+32]\n",
    "    mov rbx, [rsi+40]\n",
    // Restore RSP
    "    mov rsp, [rsi+56]\n",
    // Restore RIP and jump
    "    mov rax, [rsi+48]\n",
    "    push rax\n",
    "    ret\n",
);

unsafe extern "C" {
    /// Switch from current context to next context.
    /// 
    /// # Safety
    /// Both contexts must be valid. Interrupts should be disabled.
    pub fn switch_context(current: *mut ThreadContext, next: *const ThreadContext);
}
