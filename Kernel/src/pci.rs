//! The PCI bus.
//!
//! The kernel finds the devices of the machine through the configuration
//! space of the bus. The configuration space of a device is a page of
//! registers that describe the device: who it is, which interrupts it uses,
//! which memory it wants, and which capabilities it offers.
//!
//! # The access mechanism
//!
//! The kernel uses the mechanism of the original PCI bus: a register in the
//! port `0x0CF8` holds the address of a register of the configuration space,
//! and the port `0x0CFC` holds the data. The mechanism reaches the first 256
//! bytes of the configuration space of every device, which is where the
//! capabilities of the virtio devices live.
//!
//! # The capabilities
//!
//! A capability is a small structure in the configuration space that offers
//! a feature of the device. The capabilities form a linked list that starts
//! at the register `0x34`. A vendor-specific capability (the identifier
//! `0x09`) is the one the virtio devices use to describe where their
//! registers are.

use alloc::vec::Vec;

use core::arch::asm;

/// The port that holds the address of a register of the configuration space.
const CONFIG_ADDRESS: u16 = 0x0CF8;

/// The port that holds the data of a register of the configuration space.
const CONFIG_DATA: u16 = 0x0CFC;

/// The bit of the address register that tells the bus that the address is
/// valid.
const ADDRESS_ENABLE: u32 = 0x8000_0000;

/// The offset of the register that holds the vendor identifier.
const OFFSET_VENDOR: u16 = 0x00;

/// The offset of the register that holds the device identifier.
const OFFSET_DEVICE: u16 = 0x02;

/// The offset of the command register.
const OFFSET_COMMAND: u16 = 0x04;

/// The offset of the register that holds the first base address register.
const OFFSET_BAR: u16 = 0x10;

/// The offset of the register that holds the pointer to the capabilities.
const OFFSET_CAPABILITY: u16 = 0x34;

/// The bit of the command register that enables the memory space of the
/// device.
pub const COMMAND_MEMORY: u16 = 0x0002;

/// The bit of the command register that lets the device drive the bus.
pub const COMMAND_BUS_MASTER: u16 = 0x0004;

/// The bit of the command register that stops the device from raising an
/// interrupt.
pub const COMMAND_INTERRUPT_DISABLE: u16 = 0x0400;

/// The vendor identifier of the devices of the virtio family.
pub const VENDOR_VIRTIO: u16 = 0x1AF4;

/// The device identifier of a transitional virtio-blk device.
pub const DEVICE_VIRTIO_BLK_TRANSITIONAL: u16 = 0x1001;

/// The device identifier of a modern virtio-blk device.
pub const DEVICE_VIRTIO_BLK_MODERN: u16 = 0x1042;

/// The identifier of a vendor-specific capability.
pub const CAPABILITY_VENDOR: u8 = 0x09;

/// The number of base address registers of a device.
const BAR_COUNT: u8 = 6;

/// The largest number of capabilities a device may offer.
const MAX_CAPABILITIES: usize = 48;

/// Where a device sits on the bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    /// The number of the bus.
    pub bus: u8,
    /// The number of the device on the bus.
    pub device: u8,
    /// The number of the function of the device.
    pub function: u8,
}

impl Location {
    /// Whether the location names a device at all.
    ///
    /// A bus with a device that is not present reads back as all ones, and
    /// such a location names nothing.
    pub fn is_present(self) -> bool {
        !(self.device == 0x1F && self.function == 0x07)
    }
}

/// The kind of a base address register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarKind {
    /// The register describes a space in the memory of the processor.
    Memory,
    /// The register describes a space in the memory of the processor that
    /// holds sixty-four bits. The register behind it holds the upper half.
    Memory64,
    /// The register describes a space in the input and output space.
    InputOutput,
    /// The register is not used.
    Unused,
}

/// A capability of a device.
#[derive(Debug, Clone, Copy)]
pub struct Capability {
    /// The offset of the capability in the configuration space.
    pub offset: u16,
    /// The identifier of the capability.
    pub id: u8,
    /// The bytes of the capability.
    ///
    /// The first byte of a vendor-specific capability is its identifier, the
    /// second one the offset of the next capability, the third one its
    /// length, the fourth one its kind, and the fifth one the number of the
    /// base address register that holds it. The bytes eight to twelve hold
    /// the offset of the capability inside its register, and the bytes
    /// twelve to sixteen hold its length.
    pub data: [u8; 20],
}

impl Capability {
    /// The kind of the capability.
    ///
    /// For a vendor-specific capability the kind tells what the capability
    /// describes: the common configuration of the device, the notification
    /// register, the interrupt status, the configuration of the device, or
    /// the configuration space of the bus.
    pub fn kind(&self) -> u8 {
        self.data[3]
    }

    /// The number of the base address register that holds the capability.
    pub fn bar(&self) -> u8 {
        self.data[4]
    }

    /// The offset of the capability inside its base address register.
    pub fn offset_in_bar(&self) -> u32 {
        u32::from_le_bytes([self.data[8], self.data[9], self.data[10], self.data[11]])
    }

    /// The length of the capability.
    pub fn length(&self) -> u32 {
        u32::from_le_bytes([self.data[12], self.data[13], self.data[14], self.data[15]])
    }

    /// The multiplier of the offset of the notification register.
    ///
    /// The multiplier follows the capability of the notification register.
    /// A capability that is too close to the end of the configuration space
    /// to hold the multiplier has none.
    pub fn notify_multiplier(&self) -> u32 {
        // The multiplier is only there when the capability is long enough
        // to hold it.
        if self.data[2] < 20 {
            return 0;
        }
        u32::from_le_bytes([self.data[16], self.data[17], self.data[18], self.data[19]])
    }
}

/// A device on the PCI bus.
#[derive(Debug, Clone, Copy)]
pub struct Device {
    /// Where the device sits on the bus.
    pub location: Location,
}

impl Device {
    /// Read a byte of the configuration space.
    pub fn read8(&self, offset: u16) -> u8 {
        let word = self.read32(offset & !3);
        ((word >> (8 * (offset & 3))) & 0xFF) as u8
    }

    /// Read two bytes of the configuration space.
    pub fn read16(&self, offset: u16) -> u16 {
        let word = self.read32(offset & !3);
        ((word >> (8 * (offset & 3))) & 0xFFFF) as u16
    }

    /// Read four bytes of the configuration space.
    pub fn read32(&self, offset: u16) -> u32 {
        let address = self.address(offset);
        // SAFETY: the address register of the bus holds a valid address, and
        // the data register returns the word the address names.
        unsafe {
            out_u32(CONFIG_ADDRESS, address);
            in_u32(CONFIG_DATA)
        }
    }

    /// Write two bytes of the configuration space.
    ///
    /// The bus writes whole words, so the word that holds the bytes is read,
    /// changed, and written back.
    pub fn write16(&self, offset: u16, value: u16) {
        let aligned = offset & !3;
        let shift = 8 * (offset & 3);
        let mut word = self.read32(aligned);
        word &= !(0xFFFFu32 << shift);
        word |= (value as u32) << shift;
        self.write32(aligned, word);
    }

    /// Write four bytes of the configuration space.
    pub fn write32(&self, offset: u16, value: u32) {
        let address = self.address(offset);
        // SAFETY: the address register of the bus holds a valid address, and
        // the data register takes the word the address names.
        unsafe {
            out_u32(CONFIG_ADDRESS, address);
            out_u32(CONFIG_DATA, value);
        }
    }

    /// The address of a register of the configuration space.
    ///
    /// The address names the bus, the device, the function, and the register
    /// inside the configuration space. The register has to be aligned to four
    /// bytes: the bus reads whole words, and the offset inside the word
    /// selects the bytes that are used.
    fn address(&self, offset: u16) -> u32 {
        ADDRESS_ENABLE
            | ((self.location.bus as u32) << 16)
            | ((self.location.device as u32) << 11)
            | ((self.location.function as u32) << 8)
            | (offset as u32 & 0xFC)
    }

    /// The vendor identifier of the device.
    pub fn vendor_id(&self) -> u16 {
        self.read16(OFFSET_VENDOR)
    }

    /// The device identifier of the device.
    pub fn device_id(&self) -> u16 {
        self.read16(OFFSET_DEVICE)
    }

    /// The class code of the device (offset 0x0B).
    pub fn class_code(&self) -> u8 {
        self.read8(0x0B)
    }

    /// The subclass code of the device (offset 0x0B).
    pub fn subclass_code(&self) -> u8 {
        self.read8(0x0A)
    }

    /// The command register of the device.
    pub fn command(&self) -> u16 {
        self.read16(OFFSET_COMMAND)
    }

    /// Change the command register of the device.
    pub fn set_command(&self, command: u16) {
        self.write16(OFFSET_COMMAND, command);
    }

    /// The physical address of a base address register.
    ///
    /// The function returns `None` when the register describes a space the
    /// kernel cannot use: a register of the input and output space, or a
    /// register that is not assigned an address yet. A register that holds
    /// sixty-four bits takes the register behind it for its upper half.
    pub fn bar_address(&self, index: u8) -> Option<u64> {
        if index >= BAR_COUNT {
            return None;
        }
        let offset = OFFSET_BAR + index as u16 * 4;
        let low = self.read32(offset);
        if low & 1 != 0 {
            // The register describes the input and output space.
            return None;
        }
        let address = (low & 0xFFFF_FFF0) as u64;
        match self.bar_kind(index) {
            BarKind::Memory64 => {
                // The register holds sixty-four bits: the upper half is in
                // the register behind it.
                if index + 1 >= BAR_COUNT {
                    return None;
                }
                let high = self.read32(offset + 4) as u64;
                Some((high << 32) | address)
            }
            _ => Some(address),
        }
    }

    /// The kind of a base address register.
    pub fn bar_kind(&self, index: u8) -> BarKind {
        if index >= BAR_COUNT {
            return BarKind::Unused;
        }
        let low = self.read32(OFFSET_BAR + index as u16 * 4);
        if low & 1 != 0 {
            // The register describes the input and output space.
            return BarKind::InputOutput;
        }
        match (low >> 1) & 3 {
            0 | 1 => BarKind::Memory,
            2 => BarKind::Memory64,
            _ => BarKind::Unused,
        }
    }

    /// The capabilities of the device.
    ///
    /// The function walks the list of the capabilities and returns the ones
    /// the device offers. A device that offers no capability at all returns
    /// an empty list.
    pub fn capabilities(&self) -> Vec<Capability> {
        let mut capabilities = Vec::new();
        let mut pointer = u16::from(self.read8(OFFSET_CAPABILITY) & 0xFC);
        let mut visited = 0;

        while pointer >= 0x40 && visited < MAX_CAPABILITIES {
            let id = self.read8(pointer);
            let mut data = [0u8; 20];
            for (index, byte) in data.iter_mut().enumerate() {
                // The configuration space that the bus reaches ends after
                // 256 bytes, so the last bytes of a capability that lies
                // close to the end of the space cannot be read.
                if pointer + index as u16 > 0xFF {
                    break;
                }
                *byte = self.read8(pointer + index as u16);
            }
            capabilities.push(Capability {
                offset: pointer,
                id,
                data,
            });
            visited += 1;

            let next = u16::from(data[1] & 0xFC);
            if next == pointer || next == 0 {
                break;
            }
            pointer = next;
        }

        capabilities
    }

    /// The capabilities of the device of the virtio family.
    ///
    /// The function returns the vendor-specific capabilities of the device
    /// in the order the device offers them.
    pub fn virtio_capabilities(&self) -> Vec<Capability> {
        self.capabilities()
            .into_iter()
            .filter(|capability| capability.id == CAPABILITY_VENDOR)
            .collect()
    }
}

/// Find the devices of a vendor and a device identifier on the bus.
///
/// The function walks the devices of the first bus and returns the ones
/// that match. A device that is not present is skipped: the bus answers
/// with all ones for a device that does not exist.
pub fn find(vendor: u16, device: u16) -> Vec<Device> {
    let mut found = Vec::new();
    for bus in 0..=1u8 {
        for device_number in 0..32u8 {
            for function in 0..8u8 {
                let location = Location {
                    bus,
                    device: device_number,
                    function,
                };
                if !location.is_present() {
                    continue;
                }
                let candidate = Device { location };
                if candidate.vendor_id() == vendor && candidate.device_id() == device {
                    found.push(candidate);
                }
            }
        }
    }
    found
}

/// Find the devices of the virtio-blk family on the bus.
///
/// The function returns the transitional devices and the modern devices.
pub fn find_block_devices() -> Vec<Device> {
    let mut devices = find(VENDOR_VIRTIO, DEVICE_VIRTIO_BLK_TRANSITIONAL);
    devices.extend(find(VENDOR_VIRTIO, DEVICE_VIRTIO_BLK_MODERN));
    devices
}

/// Find PCI devices by class and subclass code.
///
/// The function walks the devices of the first bus and returns the ones
/// that match. A device that is not present is skipped.
pub fn find_by_class(class: u8, subclass: u8) -> Vec<Device> {
    let mut found = Vec::new();
    for bus in 0..=1u8 {
        for device_number in 0..32u8 {
            for function in 0..8u8 {
                let location = Location {
                    bus,
                    device: device_number,
                    function,
                };
                if !location.is_present() {
                    continue;
                }
                let candidate = Device { location };
                if candidate.class_code() == class && candidate.subclass_code() == subclass {
                    found.push(candidate);
                }
            }
        }
    }
    found
}

/// Write a word to a port.
unsafe fn out_u32(port: u16, value: u32) {
    // SAFETY: the caller promises that the port may be written.
    unsafe {
        asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack, preserves_flags));
    }
}

/// Read a word from a port.
unsafe fn in_u32(port: u16) -> u32 {
    let value: u32;
    // SAFETY: the caller promises that the port may be read.
    unsafe {
        asm!("in eax, dx", out("eax") value, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    value
}
