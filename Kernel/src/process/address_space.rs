//! Per-process address space (stub).

#![allow(dead_code)]

use crate::process::process::AddressSpace;
use crate::pmm;
use crate::serial;

impl AddressSpace {
    pub fn create() -> Self {
        Self::new(0)
    }

    /// Clean up user address space mappings and release owned frames.
    /// Preserves kernel mappings.
    pub fn cleanup(&mut self, _phys_offset: u64) {
        unsafe {
            serial::write_str("Elvaris kernel: process: cleaning up address space\r\n");
        }
        // Release tracked owned frames
        for i in 0..self.owned_frames_count {
            let frame = self.owned_frames[i];
            if frame != 0 {
                pmm::with_allocator(|alloc| {
                    let _ = alloc.deallocate_frame(frame);
                });
            }
        }
        self.owned_frames_count = 0;
        self.owned_count = 0;
        self.user_pages = 0;
    }
}
