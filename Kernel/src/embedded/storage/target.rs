//! Storage target selection.
//!
//! Selects the primary storage path for embedded deployment. For QEMU-first
//! foundation, VirtIO block is the natural target.

use alloc::string::{String, ToString};
use core::fmt;

/// The type of storage target available on the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageTargetType {
    /// QEMU VirtIO block device.
    VirtioBlock,
    /// SD/eMMC card.
    SdEmmc,
    /// SPI flash.
    SpiFlash,
    /// NVMe SSD.
    Nvme,
    /// SATA drive.
    Sata,
    /// USB storage.
    UsbStorage,
    /// No physical storage (memory-only).
    MemoryOnly,
}

/// A selected storage target with its characteristics.
pub struct StorageTarget {
    /// The type of the target.
    pub target_type: StorageTargetType,
    /// Whether the target is available on this device.
    pub available: bool,
    /// The total capacity in bytes.
    pub capacity_bytes: u64,
    /// The block size in bytes.
    pub block_size: u64,
    /// Whether the target supports wear leveling.
    pub wear_leveling: bool,
    /// Whether the target supports power-loss protection.
    pub power_loss_protection: bool,
}

impl StorageTarget {
    /// Create a new storage target.
    pub fn new(target_type: StorageTargetType) -> Self {
        let (capacity, block_size, wear, plp) = match target_type {
            StorageTargetType::VirtioBlock => (32 * 1024 * 1024 * 1024, 512, true, false),
            StorageTargetType::SdEmmc => (16 * 1024 * 1024 * 1024, 512, true, false),
            StorageTargetType::SpiFlash => (8 * 1024 * 1024, 4096, false, false),
            StorageTargetType::Nvme => (256 * 1024 * 1024 * 1024, 512, true, true),
            StorageTargetType::Sata => (500 * 1024 * 1024 * 1024, 512, false, false),
            StorageTargetType::UsbStorage => (64 * 1024 * 1024 * 1024, 512, false, false),
            StorageTargetType::MemoryOnly => (0, 512, false, false),
        };
        Self {
            target_type,
            available: target_type != StorageTargetType::MemoryOnly,
            capacity_bytes: capacity,
            block_size,
            wear_leveling: wear,
            power_loss_protection: plp,
        }
    }

    /// Whether the target is suitable for embedded deployment (has at least
    /// basic reliability characteristics).
    pub fn suitable_for_embedded(&self) -> bool {
        self.available && self.capacity_bytes >= 4 * 1024 * 1024 * 1024
    }

    /// The short name of the target type.
    pub fn name(&self) -> &'static str {
        match self.target_type {
            StorageTargetType::VirtioBlock => "virtio-block",
            StorageTargetType::SdEmmc => "sd-emmc",
            StorageTargetType::SpiFlash => "spi-flash",
            StorageTargetType::Nvme => "nvme",
            StorageTargetType::Sata => "sata",
            StorageTargetType::UsbStorage => "usb-storage",
            StorageTargetType::MemoryOnly => "memory-only",
        }
    }
}

impl fmt::Display for StorageTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl Default for StorageTarget {
    fn default() -> Self {
        Self::new(StorageTargetType::VirtioBlock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtio_block_is_default() {
        let target = StorageTarget::default();
        assert_eq!(target.target_type, StorageTargetType::VirtioBlock);
        assert!(target.available);
        assert!(target.suitable_for_embedded());
    }

    #[test]
    fn memory_only_not_available() {
        let target = StorageTarget::new(StorageTargetType::MemoryOnly);
        assert!(!target.available);
        assert!(!target.suitable_for_embedded());
    }

    #[test]
    fn spi_flash_too_small() {
        let target = StorageTarget::new(StorageTargetType::SpiFlash);
        assert!(!target.suitable_for_embedded());
    }

    #[test]
    fn nvme_has_power_loss_protection() {
        let target = StorageTarget::new(StorageTargetType::Nvme);
        assert!(target.power_loss_protection);
        assert!(target.suitable_for_embedded());
    }

    #[test]
    fn target_names() {
        assert_eq!(StorageTarget::new(StorageTargetType::VirtioBlock).name(), "virtio-block");
        assert_eq!(StorageTarget::new(StorageTargetType::SdEmmc).name(), "sd-emmc");
        assert_eq!(StorageTarget::new(StorageTargetType::MemoryOnly).name(), "memory-only");
    }
}
