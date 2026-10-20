//! Context switching implementation for x86_64.

use crate::threads::context::ThreadContext;
use core::arch::global_asm;

global_asm!(
    ".section .text\n",
    ".globl switch_context\n",
    "switch_context:\n",
    // Save current context (callee-saved registers)
    // RDI = current context, RSI = next context
    "    movq %r15, 0(%rdi)\n",
    "    movq %r14, 8(%rdi)\n",
    "    movq %r13, 16(%rdi)\n",
    "    movq %r12, 24(%rdi)\n",
    "    movq %rbp, 32(%rdi)\n",
    "    movq %rbx, 40(%rdi)\n",
    // Save RIP (return address)
    "    movq (%rsp), %rax\n",
    "    movq %rax, 48(%rdi)\n",
    // Save RSP
    "    leaq 8(%rsp), %rax\n",
    "    movq %rax, 56(%rdi)\n",
    // Restore next context
    "    movq 0(%rsi), %r15\n",
    "    movq 8(%rsi), %r14\n",
    "    movq 16(%rsi), %r13\n",
    "    movq 24(%rsi), %r12\n",
    "    movq 32(%rsi), %rbp\n",
    "    movq 40(%rsi), %rbx\n",
    // Restore RSP
    "    movq 56(%rsi), %rsp\n",
    // Restore RIP and jump
    "    movq 48(%rsi), %rax\n",
    "    pushq %rax\n",
    "    ret\n",
);

unsafe extern "C" {
    /// Switch from current context to next context.
    /// 
    /// # Safety
    /// Both contexts must be valid. Interrupts should be disabled.
    pub fn switch_context(current: *mut ThreadContext, next: *const ThreadContext);
}
