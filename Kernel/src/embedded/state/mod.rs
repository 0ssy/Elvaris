//! Read-only system and writable state separation.
//!
//! This module implements the logical separation between immutable system
//! content and mutable application/device state, as required by embedded
//! deployments.
//!
//! # Storage layout
//!
//! ```text
//! /system    Immutable system area (kernel, binaries, libraries, config)
//! /state     Mutable writable state (app data, logs, device identity)
//! /recovery  Recovery mode data (previous images, rollback metadata)
//! /staging   Update staging area (incoming updates before activation)
//! ```
//!
//! The system area is mounted read-only and protected from ordinary
//! application writes. The writable state area is separate and can be
//! written by applications with appropriate permissions.

pub mod layout;
pub mod mount;
pub mod protection;

pub use layout::StorageArea;
pub use mount::StateMount;
pub use protection::WriteProtector;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// Errors of the state subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateError {
    /// A storage area is not known.
    UnknownArea,
    /// A storage area is not mounted.
    NotMounted,
    /// A write was attempted on a read-only area.
    ReadOnly,
    /// A state area is corrupted.
    Corrupt,
    /// Insufficient space in a state area.
    InsufficientSpace,
    /// The state area is unavailable.
    Unavailable,
}

impl StateError {
    pub fn description(self) -> &'static str {
        match self {
            StateError::UnknownArea => "unknown storage area",
            StateError::NotMounted => "storage area not mounted",
            StateError::ReadOnly => "storage area is read-only",
            StateError::Corrupt => "storage area is corrupt",
            StateError::InsufficientSpace => "insufficient space in storage area",
            StateError::Unavailable => "storage area unavailable",
        }
    }
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}

/// Whether a state area is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaAvailability {
    /// The area is present and usable.
    Available,
    /// The area is present but missing (first boot).
    Missing,
    /// The area is present but failed validation.
    Failed,
    /// The area is not present on this device.
    Absent,
}
