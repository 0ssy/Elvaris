//! Storage layout with partitions and slots.
//!
//! Documents the storage layout used by an embedded Elvaris system.

use super::StorageError;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// A partition on the storage device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    /// The name of the partition.
    pub name: String,
    /// The starting offset in bytes.
    pub start_offset: u64,
    /// The size in bytes.
    pub size_bytes: u64,
    /// The purpose of the partition.
    pub purpose: PartitionPurpose,
}

/// The purpose of a partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionPurpose {
    /// Boot loader and kernel image.
    Boot,
    /// Immutable system area (slot A).
    SystemSlotA,
    /// Immutable system area (slot B, for A/B updates).
    SystemSlotB,
    /// Mutable writable state.
    State,
    /// Recovery mode data.
    Recovery,
    /// Update staging area.
    Staging,
}

/// The complete storage layout of an embedded device.
pub struct StorageLayout {
    /// The total device capacity in bytes.
    pub total_capacity: u64,
    /// The block size in bytes.
    pub block_size: u64,
    /// All partitions, in order.
    pub partitions: Vec<Partition>,
}

impl StorageLayout {
    /// Create a default embedded storage layout for a 32 GiB VirtIO block
    /// device.
    pub fn default_embedded() -> Self {
        let total = 32u64 * 1024 * 1024 * 1024;
        let block_size = 512u64;
        let boot_size = 64u64 * 1024 * 1024;
        let system_size = 4u64 * 1024 * 1024 * 1024;
        let state_size = 4u64 * 1024 * 1024 * 1024;
        let recovery_size = 1u64 * 1024 * 1024 * 1024;
        let staging_size = 2u64 * 1024 * 1024 * 1024;

        let mut partitions = Vec::new();
        let mut offset = 0u64;

        partitions.push(Partition {
            name: "boot".to_string(),
            start_offset: offset,
            size_bytes: boot_size,
            purpose: PartitionPurpose::Boot,
        });
        offset += boot_size;

        partitions.push(Partition {
            name: "system-a".to_string(),
            start_offset: offset,
            size_bytes: system_size,
            purpose: PartitionPurpose::SystemSlotA,
        });
        offset += system_size;

        partitions.push(Partition {
            name: "system-b".to_string(),
            start_offset: offset,
            size_bytes: system_size,
            purpose: PartitionPurpose::SystemSlotB,
        });
        offset += system_size;

        partitions.push(Partition {
            name: "state".to_string(),
            start_offset: offset,
            size_bytes: state_size,
            purpose: PartitionPurpose::State,
        });
        offset += state_size;

        partitions.push(Partition {
            name: "recovery".to_string(),
            start_offset: offset,
            size_bytes: recovery_size,
            purpose: PartitionPurpose::Recovery,
        });
        offset += recovery_size;

        partitions.push(Partition {
            name: "staging".to_string(),
            start_offset: offset,
            size_bytes: staging_size,
            purpose: PartitionPurpose::Staging,
        });

        Self {
            total_capacity: total,
            block_size,
            partitions,
        }
    }

    /// Find a partition by its purpose.
    pub fn find_by_purpose(&self, purpose: PartitionPurpose) -> Option<&Partition> {
        self.partitions.iter().find(|p| p.purpose == purpose)
    }

    /// The system slot A partition.
    pub fn system_slot_a(&self) -> Option<&Partition> {
        self.find_by_purpose(PartitionPurpose::SystemSlotA)
    }

    /// The system slot B partition.
    pub fn system_slot_b(&self) -> Option<&Partition> {
        self.find_by_purpose(PartitionPurpose::SystemSlotB)
    }

    /// The state partition.
    pub fn state_partition(&self) -> Option<&Partition> {
        self.find_by_purpose(PartitionPurpose::State)
    }

    /// Validate the layout: partitions don't overlap and fit within capacity.
    pub fn validate(&self) -> Result<(), StorageError> {
        let mut last_end = 0u64;
        for partition in &self.partitions {
            if partition.start_offset < last_end {
                return Err(StorageError::CorruptMetadata);
            }
            if partition.start_offset + partition.size_bytes > self.total_capacity {
                return Err(StorageError::InsufficientSpace);
            }
            last_end = partition.start_offset + partition.size_bytes;
        }
        Ok(())
    }
}

impl Default for StorageLayout {
    fn default() -> Self {
        Self::default_embedded()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_has_all_partitions() {
        let layout = StorageLayout::default_embedded();
        assert_eq!(layout.partitions.len(), 6);
        assert!(layout.find_by_purpose(PartitionPurpose::Boot).is_some());
        assert!(layout.find_by_purpose(PartitionPurpose::SystemSlotA).is_some());
        assert!(layout.find_by_purpose(PartitionPurpose::SystemSlotB).is_some());
        assert!(layout.find_by_purpose(PartitionPurpose::State).is_some());
        assert!(layout.find_by_purpose(PartitionPurpose::Recovery).is_some());
        assert!(layout.find_by_purpose(PartitionPurpose::Staging).is_some());
    }

    #[test]
    fn layout_validates() {
        let layout = StorageLayout::default_embedded();
        assert!(layout.validate().is_ok());
    }

    #[test]
    fn overlapping_partitions_rejected() {
        let mut layout = StorageLayout::default_embedded();
        // Make system-a overlap with system-b
        if let Some(slot_a) = layout.partitions.iter_mut().find(|p| p.name == "system-a") {
            slot_a.size_bytes = slot_a.size_bytes * 2;
        }
        assert!(layout.validate().is_err());
    }

    #[test]
    fn partition_exceeds_capacity_rejected() {
        let mut layout = StorageLayout::default_embedded();
        if let Some(boot) = layout.partitions.iter_mut().find(|p| p.name == "boot") {
            boot.size_bytes = layout.total_capacity;
        }
        assert!(layout.validate().is_err());
    }

    #[test]
    fn system_slots_found() {
        let layout = StorageLayout::default_embedded();
        assert!(layout.system_slot_a().is_some());
        assert!(layout.system_slot_b().is_some());
        assert_eq!(layout.system_slot_a().unwrap().name, "system-a");
        assert_eq!(layout.system_slot_b().unwrap().name, "system-b");
    }
}
