//! Thread stack allocation.

extern crate alloc;
use alloc::vec::Vec;
use core::alloc::Layout;

/// Default kernel stack size (64 KiB).
pub const KERNEL_STACK_SIZE: usize = 64 * 1024;

/// Stack alignment requirement (16 bytes for x86_64).
pub const STACK_ALIGNMENT: usize = 16;

/// A kernel stack.
#[repr(C, align(16))]
#[derive(Clone)]
pub struct KernelStack {
    data: Vec<u8>,
}

impl KernelStack {
    /// Allocate a new kernel stack.
    pub fn new() -> Option<Self> {
        let _layout = Layout::from_size_align(KERNEL_STACK_SIZE, STACK_ALIGNMENT).ok()?;
        let data = vec![0u8; KERNEL_STACK_SIZE];
        Some(Self { data })
    }

    /// Get the top of the stack (stacks grow downward on x86_64).
    pub fn top(&self) -> u64 {
        let ptr = self.data.as_ptr() as u64;
        ptr + self.data.len() as u64
    }

    /// Get bottom of stack.
    pub fn bottom(&self) -> u64 {
        self.data.as_ptr() as u64
    }

    /// Get size.
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

impl Default for KernelStack {
    fn default() -> Self {
        Self::new().expect("failed to allocate kernel stack")
    }
}
