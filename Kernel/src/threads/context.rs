//! Saved CPU context for x86_64 thread switching.
//!
//! The context contains callee-saved registers and control state.
//! Caller-saved registers are preserved by the System V calling convention.

use core::mem::size_of;

/// Size of the saved context in bytes.
pub const CONTEXT_SIZE: usize = size_of::<ThreadContext>();

/// Saved CPU context for a kernel thread.
///
/// Callee-saved registers (must be preserved by switch):
/// - RBX, RBP, R12, R13, R14, R15
///
/// Control state:
/// - RSP (stack pointer)
/// - RIP (instruction pointer)
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, Default)]
pub struct ThreadContext {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rip: u64,
    pub rsp: u64,
}
