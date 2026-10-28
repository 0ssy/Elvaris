//! Kernel threads and cooperative context switching.

#![allow(dead_code)]

pub mod context;
pub mod stack;
pub mod switch;
pub mod scheduler;

#[cfg(test)]
mod tests;

use alloc::string::String;
extern crate alloc;

pub use context::ThreadContext;
pub use stack::KernelStack;

/// Thread identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadId(pub u64);

/// Thread state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Created,
    Runnable,
    Running,
    Blocked,
    Terminated,
}

/// Thread entry function.
pub type ThreadEntry = extern "C" fn(*mut u8) -> !;

/// A kernel thread.
#[derive(Clone)]
pub struct Thread {
    pub id: ThreadId,
    pub state: ThreadState,
    pub context: ThreadContext,
    pub stack: Option<KernelStack>,
    pub entry: Option<ThreadEntry>,
    pub arg: *mut u8,
    pub name: Option<String>,
}

unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

impl Thread {
    /// Create a new thread.
    pub fn new(entry: ThreadEntry, arg: *mut u8, name: Option<String>) -> Self {
        let stack = KernelStack::new().expect("failed to allocate stack");
        let stack_top = stack.top();
        
        // Initialize context
        let mut context = ThreadContext::default();
        context.rsp = stack_top & !0xF;
        context.rip = entry as u64;
        
        Thread {
            id: ThreadId({
                use core::sync::atomic::{AtomicU64, Ordering};
                static NEXT_ID: AtomicU64 = AtomicU64::new(1);
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            }),
            state: ThreadState::Runnable,
            context,
            stack: Some(stack),
            entry: Some(entry),
            arg,
            name,
        }
    }
    
    /// Get thread name for diagnostics.
    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("unnamed")
    }
}
