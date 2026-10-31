//! Embedded storage behavior.
//!
//! Makes storage behavior reliable and predictable under embedded
//! constraints. Builds on the logical storage layout from Phase 20.6.
//!
//! # Storage behavior
//!
//! ```text
//! Discover storage
//!     ↓
//! Define layout (boot, system, state, recovery, staging)
//!     ↓
//! Track failure classes (unavailable, read, write, timeout, corrupt)
//!     ↓
//! Atomic state updates (write temp, flush, verify, rename)
//!     ↓
//! Power-loss behavior (documented for each operation)
//!     ↓
//! Storage health (read errors, write errors, timeouts, space)
//! ```

pub mod target;
pub mod layout;
pub mod failure;
pub mod atomic;
pub mod health;

pub use target::StorageTarget;
pub use layout::StorageLayout;
pub use failure::StorageFailure;
pub use atomic::AtomicStateUpdate;
pub use health::StorageHealth;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// Errors of the storage subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    /// The storage device is not available.
    Unavailable,
    /// A read operation failed.
    ReadFailed,
    /// A write operation failed.
    WriteFailed,
    /// An operation timed out.
    Timeout,
    /// Metadata is corrupt.
    CorruptMetadata,
    /// The filesystem is corrupt.
    CorruptFilesystem,
    /// Insufficient space.
    InsufficientSpace,
    /// The partition is invalid.
    InvalidPartition,
}

impl StorageError {
    pub fn description(self) -> &'static str {
        match self {
            StorageError::Unavailable => "storage device unavailable",
            StorageError::ReadFailed => "storage read failed",
            StorageError::WriteFailed => "storage write failed",
            StorageError::Timeout => "storage operation timed out",
            StorageError::CorruptMetadata => "storage metadata is corrupt",
            StorageError::CorruptFilesystem => "storage filesystem is corrupt",
            StorageError::InsufficientSpace => "insufficient storage space",
            StorageError::InvalidPartition => "invalid storage partition",
        }
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}
