//! Storage area layout definitions.
//!
//! Defines the logical storage areas used by an embedded Elvaris system:
//! system (immutable), state (mutable), recovery, and staging.

use alloc::string::{String, ToString};
use core::fmt;

/// A logical storage area of the embedded system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StorageArea {
    /// The immutable system area: kernel, system binaries, libraries,
    /// default configuration, service manifests.
    System,
    /// The mutable writable state: application data, logs, device identity,
    /// update metadata, recovery state.
    State,
    /// The recovery area: previous known-good images, rollback metadata,
    /// recovery tools, safe-mode configuration.
    Recovery,
    /// The update staging area: incoming updates staged before activation.
    Staging,
}

impl StorageArea {
    /// The mount path of the storage area.
    pub fn path(self) -> &'static str {
        match self {
            StorageArea::System => "/system",
            StorageArea::State => "/state",
            StorageArea::Recovery => "/recovery",
            StorageArea::Staging => "/staging",
        }
    }

    /// The short name of the storage area, used in diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            StorageArea::System => "system",
            StorageArea::State => "state",
            StorageArea::Recovery => "recovery",
            StorageArea::Staging => "staging",
        }
    }

    /// Whether the area is immutable (read-only by design).
    pub fn is_immutable(self) -> bool {
        matches!(self, StorageArea::System)
    }

    /// Whether the area is required for normal boot.
    pub fn is_required_for_boot(self) -> bool {
        matches!(self, StorageArea::System) | matches!(self, StorageArea::State)
    }
}

impl fmt::Display for StorageArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// All storage areas in mount order (system first, then state).
pub const ALL_AREAS: [StorageArea; 4] = [
    StorageArea::System,
    StorageArea::State,
    StorageArea::Recovery,
    StorageArea::Staging,
];

/// Areas that must be mounted before services can start.
pub const BOOT_AREAS: [StorageArea; 2] = [
    StorageArea::System,
    StorageArea::State,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_paths() {
        assert_eq!(StorageArea::System.path(), "/system");
        assert_eq!(StorageArea::State.path(), "/state");
        assert_eq!(StorageArea::Recovery.path(), "/recovery");
        assert_eq!(StorageArea::Staging.path(), "/staging");
    }

    #[test]
    fn area_immutability() {
        assert!(StorageArea::System.is_immutable());
        assert!(!StorageArea::State.is_immutable());
        assert!(!StorageArea::Recovery.is_immutable());
        assert!(!StorageArea::Staging.is_immutable());
    }

    #[test]
    fn area_boot_requirement() {
        assert!(StorageArea::System.is_required_for_boot());
        assert!(StorageArea::State.is_required_for_boot());
        assert!(!StorageArea::Recovery.is_required_for_boot());
        assert!(!StorageArea::Staging.is_required_for_boot());
    }

    #[test]
    fn all_areas_count() {
        assert_eq!(ALL_AREAS.len(), 4);
    }

    #[test]
    fn boot_areas_count() {
        assert_eq!(BOOT_AREAS.len(), 2);
    }
}
