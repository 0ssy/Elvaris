//! Per-process address space (stub).

#![allow(dead_code)]

use crate::process::process::AddressSpace;

impl AddressSpace {
    pub fn create() -> Self {
        Self::new(0)
    }
}
