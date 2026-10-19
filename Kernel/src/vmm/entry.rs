//! Page table entry definitions.

#![allow(dead_code)]

/// Page table entry flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags(u64);

impl Flags {
    pub const PRESENT: u64 = 1 << 0;
    pub const WRITABLE: u64 = 1 << 1;
    pub const USER_ACCESSIBLE: u64 = 1 << 2;
    pub const WRITE_THROUGH: u64 = 1 << 3;
    pub const NO_CACHE: u64 = 1 << 4;
    pub const ACCESSED: u64 = 1 << 5;
    pub const DIRTY: u64 = 1 << 6;
    pub const HUGE_PAGE: u64 = 1 << 7;
    pub const GLOBAL: u64 = 1 << 8;
    pub const NO_EXECUTE: u64 = 1 << 63;
}

impl Flags {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn new(flags: u64) -> Self {
        Self(flags)
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn contains(self, flag: u64) -> bool {
        (self.0 & flag) != 0
    }

    pub const fn set(self, flag: u64) -> Self {
        Self(self.0 | flag)
    }
}
