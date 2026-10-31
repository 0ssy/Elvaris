//! Mount management for storage areas.
//!
//! Tracks which storage areas are mounted, whether they are read-only or
//! writable, and enforces the mount ordering required for boot.

use super::layout::{ALL_AREAS, BOOT_AREAS, StorageArea};
use super::StateError;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use core::fmt;

/// The mount state of a storage area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountState {
    /// The area is not mounted.
    Unmounted,
    /// The area is mounted read-only.
    ReadOnly,
    /// The area is mounted read-write.
    ReadWrite,
}

impl MountState {
    pub fn as_str(self) -> &'static str {
        match self {
            MountState::Unmounted => "unmounted",
            MountState::ReadOnly => "read-only",
            MountState::ReadWrite => "read-write",
        }
    }
}

/// The mount manager for all storage areas.
pub struct StateMount {
    /// The mount state of each area.
    mounts: BTreeMap<StorageArea, MountState>,
    /// Whether the system area passed validation.
    system_valid: bool,
    /// Whether the state area passed validation.
    state_valid: bool,
    /// Whether boot is complete (boot areas mounted and validated).
    boot_complete: bool,
}

impl StateMount {
    /// Create a new mount manager with all areas unmounted.
    pub fn new() -> Self {
        let mut mounts = BTreeMap::new();
        for area in ALL_AREAS {
            mounts.insert(area, MountState::Unmounted);
        }
        Self {
            mounts,
            system_valid: false,
            state_valid: false,
            boot_complete: false,
        }
    }

    /// The mount state of an area.
    pub fn mount_state(&self, area: StorageArea) -> MountState {
        self.mounts.get(&area).copied().unwrap_or(MountState::Unmounted)
    }

    /// Whether an area is mounted.
    pub fn is_mounted(&self, area: StorageArea) -> bool {
        matches!(
            self.mounts.get(&area).copied().unwrap_or(MountState::Unmounted),
            MountState::ReadOnly | MountState::ReadWrite
        )
    }

    /// Whether the system area is mounted read-only.
    pub fn is_system_read_only(&self) -> bool {
        self.mounts.get(&StorageArea::System) == Some(&MountState::ReadOnly)
    }

    /// Whether the state area is mounted read-write.
    pub fn is_state_writable(&self) -> bool {
        self.mounts.get(&StorageArea::State) == Some(&MountState::ReadWrite)
    }

    /// Whether boot areas are mounted and validated.
    pub fn is_boot_complete(&self) -> bool {
        self.boot_complete
    }

    /// Mount the system area read-only.
    ///
    /// The system area must be mounted read-only before any service starts.
    pub fn mount_system_read_only(&mut self) -> Result<(), StateError> {
        if self.mounts[&StorageArea::System] != MountState::Unmounted {
            return Err(StateError::UnknownArea);
        }
        self.mounts.insert(StorageArea::System, MountState::ReadOnly);
        self.system_valid = true;
        Ok(())
    }

    /// Mount the state area read-write.
    ///
    /// The state area must be mounted after the system area.
    pub fn mount_state_read_write(&mut self) -> Result<(), StateError> {
        if self.mounts[&StorageArea::State] != MountState::Unmounted {
            return Err(StateError::UnknownArea);
        }
        if !self.system_valid {
            return Err(StateError::Corrupt);
        }
        self.mounts.insert(StorageArea::State, MountState::ReadWrite);
        self.state_valid = true;
        Ok(())
    }

    /// Mount the recovery area read-only.
    pub fn mount_recovery_read_only(&mut self) -> Result<(), StateError> {
        if self.mounts[&StorageArea::Recovery] != MountState::Unmounted {
            return Err(StateError::UnknownArea);
        }
        self.mounts.insert(StorageArea::Recovery, MountState::ReadOnly);
        Ok(())
    }

    /// Mount the staging area read-write.
    pub fn mount_staging_read_write(&mut self) -> Result<(), StateError> {
        if self.mounts[&StorageArea::Staging] != MountState::Unmounted {
            return Err(StateError::UnknownArea);
        }
        self.mounts.insert(StorageArea::Staging, MountState::ReadWrite);
        Ok(())
    }

    /// Mount all boot areas in order: system read-only, then state read-write.
    ///
    /// This is the mount sequence required before services can start.
    pub fn mount_boot_areas(&mut self) -> Result<(), StateError> {
        self.mount_system_read_only()?;
        self.mount_state_read_write()?;
        self.boot_complete = true;
        Ok(())
    }

    /// Unmount all areas in reverse order (staging, recovery, state, system).
    pub fn unmount_all(&mut self) -> Result<(), StateError> {
        for area in ALL_AREAS.iter().rev() {
            self.mounts.insert(*area, MountState::Unmounted);
        }
        self.system_valid = false;
        self.state_valid = false;
        self.boot_complete = false;
        Ok(())
    }

    /// Whether all boot areas are mounted.
    pub fn boot_areas_mounted(&self) -> bool {
        BOOT_AREAS.iter().all(|a| self.is_mounted(*a))
    }
}

impl Default for StateMount {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_areas_start_unmounted() {
        let mount = StateMount::new();
        for area in ALL_AREAS {
            assert_eq!(mount.mount_state(area), MountState::Unmounted);
        }
    }

    #[test]
    fn mount_system_read_only() {
        let mut mount = StateMount::new();
        mount.mount_system_read_only().unwrap();
        assert_eq!(mount.mount_state(StorageArea::System), MountState::ReadOnly);
        assert!(mount.is_system_read_only());
    }

    #[test]
    fn mount_state_requires_system_first() {
        let mut mount = StateMount::new();
        let result = mount.mount_state_read_write();
        assert!(matches!(result, Err(StateError::Corrupt)));
    }

    #[test]
    fn mount_boot_areas_in_order() {
        let mut mount = StateMount::new();
        mount.mount_boot_areas().unwrap();
        assert!(mount.is_system_read_only());
        assert!(mount.is_state_writable());
        assert!(mount.is_boot_complete());
        assert!(mount.boot_areas_mounted());
    }

    #[test]
    fn unmount_all() {
        let mut mount = StateMount::new();
        mount.mount_boot_areas().unwrap();
        mount.unmount_all().unwrap();
        assert!(!mount.is_boot_complete());
        assert_eq!(mount.mount_state(StorageArea::System), MountState::Unmounted);
        assert_eq!(mount.mount_state(StorageArea::State), MountState::Unmounted);
    }
}
