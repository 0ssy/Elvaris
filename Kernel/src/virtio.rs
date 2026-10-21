//! The modern virtio interface.
//!
//! A virtio device is a device that does the work of a driver in the host:
//! the driver of the device is the driver of the kernel, and the device
//! itself only moves the data. The kernel talks to the device through three
//! regions of the configuration space of the device:
//!
//! - the **common configuration**, a set of registers that describe the
//!   device and the state of its virtqueues,
//! - the **notification register**, which the driver writes to when it makes
//!   a request available,
//! - the **configuration of the device**, which holds the parameters of the
//!   device, like the capacity of a disk.
//!
//! The driver finds these regions through the capabilities of the device on
//! the PCI bus (see [`crate::pci`]).
//!
//! # The virtqueue
//!
//! The requests travel through a virtqueue. The queue is made of three
//! parts, each of them physically contiguous:
//!
//! - the **descriptor table** describes the buffers of the requests,
//! - the **available ring** names the descriptors the driver offers,
//! - the **used ring** names the descriptors the device has finished.
//!
//! The driver fills in a descriptor, names it in the available ring, and
//! writes the number of the queue into the notification register. The device
//! does the work and names the descriptor in the used ring.
//!
//! # The limits
//!
//! - The driver does not use interrupts: it waits for the used ring instead.
//!   The device is told not to raise an interrupt at all.
//! - Only the split form of the virtqueue is used, and only the version 1.0
//!   of the interface. The older interface of the transitional devices is
//!   not supported.

use core::sync::atomic::{AtomicU16, Ordering, fence};

use crate::pci::Device;
use crate::vmm::mapper::{MapError, Mapper};

/// The kind of the capability that describes the common configuration.
const CAP_COMMON_CONFIG: u8 = 1;

/// The kind of the capability that describes the notification register.
const CAP_NOTIFY: u8 = 2;

/// The kind of the capability that describes the configuration of the device.
const CAP_DEVICE_CONFIG: u8 = 4;

/// The offset of the register that selects the feature bits of the device.
const OFFSET_FEATURE_SELECT: usize = 0x00;

/// The offset of the register that holds the feature bits of the device.
const OFFSET_DEVICE_FEATURE: usize = 0x04;

/// The offset of the register that selects the feature bits of the driver.
const OFFSET_DRIVER_FEATURE_SELECT: usize = 0x08;

/// The offset of the register that holds the feature bits of the driver.
const OFFSET_DRIVER_FEATURE: usize = 0x0C;

/// The offset of the register that holds the number of the virtqueues.
const OFFSET_QUEUE_COUNT: usize = 0x12;

/// The offset of the register that holds the status of the device.
const OFFSET_STATUS: usize = 0x14;

/// The offset of the register that selects the virtqueue.
const OFFSET_QUEUE_SELECT: usize = 0x16;

/// The offset of the register that holds the size of the virtqueue.
const OFFSET_QUEUE_SIZE: usize = 0x18;

/// The offset of the register that holds the vector of the virtqueue.
const OFFSET_QUEUE_VECTOR: usize = 0x1A;

/// The offset of the register that enables the virtqueue.
const OFFSET_QUEUE_ENABLE: usize = 0x1C;

/// The offset of the register that holds the offset of the notification
/// register of the virtqueue.
const OFFSET_QUEUE_NOTIFY: usize = 0x1E;

/// The offset of the register that holds the address of the descriptor table.
const OFFSET_QUEUE_DESC: usize = 0x20;

/// The offset of the register that holds the address of the available ring.
const OFFSET_QUEUE_DRIVER: usize = 0x28;

/// The offset of the register that holds the address of the used ring.
const OFFSET_QUEUE_DEVICE: usize = 0x30;

/// The status bit that says that the driver has seen the device.
pub const STATUS_ACKNOWLEDGE: u8 = 1;

/// The status bit that says that the driver knows how to drive the device.
pub const STATUS_DRIVER: u8 = 2;

/// The status bit that says that the driver has finished the setup.
pub const STATUS_DRIVER_OK: u8 = 4;

/// The status bit that says that the features have been negotiated.
pub const STATUS_FEATURES_OK: u8 = 8;

/// The status bit that says that the driver has given up on the device.
pub const STATUS_FAILED: u8 = 0x80;

/// The feature bit that says that the device speaks the version 1.0 of the
/// interface.
pub const FEATURE_VERSION_1: u64 = 1 << 32;

/// The size of one descriptor of the descriptor table.
const DESCRIPTOR_SIZE: u64 = 16;

/// The size of the header of the available ring, before the ring itself.
const AVAIL_HEADER_SIZE: u64 = 4;

/// The size of the header of the used ring, before the ring itself.
const USED_HEADER_SIZE: u64 = 4;

/// The size of one entry of the used ring.
const USED_ENTRY_SIZE: u64 = 8;

/// The flag of a descriptor that continues in the next descriptor.
const DESCRIPTOR_NEXT: u16 = 1;

/// The flag of a descriptor that the device writes into.
const DESCRIPTOR_WRITE: u16 = 2;

/// The base of the memory the driver uses for the registers of the devices.
const DEVICE_MEMORY_BASE: u64 = 0xFFFF_C000_0000_0000;

/// The size of the memory one base address register may use.
const DEVICE_MEMORY_STRIDE: u64 = 0x0001_0000;

/// The size of the memory one device may use for its registers.
const DEVICE_MEMORY_SLOT: u64 = 0x0010_0000;

/// The number of spins the driver waits for the device before it gives up.
const COMPLETION_SPINS: u64 = 50_000_000;

/// The errors the driver reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioError {
    /// The device does not offer the capabilities the driver needs.
    NoCapabilities,
    /// The device does not speak the version 1.0 of the interface.
    NoModernInterface,
    /// The device refused the feature bits of the driver.
    FeaturesRefused,
    /// The device has no virtqueue.
    NoQueue,
    /// The registers of the device could not be mapped.
    MappingFailed(MapError),
    /// The device did not finish the request in time.
    Timeout,
}

/// The modern interface of a virtio device.
pub struct Transport {
    /// The registers of the common configuration.
    common: *mut u8,
    /// The configuration of the device.
    device_config: *const u8,
    /// The number of virtqueues of the device.
    queue_count: u16,
    /// The capability that describes the register of the notifications.
    notify_capability: crate::pci::Capability,
    /// The base address of the register of every base address register.
    bars: [u64; 6],
    /// The base of the memory that holds the registers of this device.
    device_base: u64,
}

impl Transport {
    /// Prepare the device of a PCI device for the modern interface.
    ///
    /// The function maps the registers of the device into the memory of the
    /// kernel and negotiates the features of the device. The virtqueues are
    /// prepared later, one by one.
    ///
    /// The `wanted` features are the feature bits of the device the driver
    /// understands. A bit the device does not offer is not accepted.
    pub fn new(
        device: &Device,
        mapper: &mut Mapper,
        wanted: u64,
        slot: usize,
    ) -> Result<Self, VirtioError> {
        let capabilities = device.virtio_capabilities();
        let common_cap = capabilities
            .iter()
            .find(|capability| capability.kind() == CAP_COMMON_CONFIG)
            .copied()
            .ok_or(VirtioError::NoCapabilities)?;
        let notify_cap = capabilities
            .iter()
            .find(|capability| capability.kind() == CAP_NOTIFY)
            .copied()
            .ok_or(VirtioError::NoCapabilities)?;

        // The device has to be able to reach the memory of the kernel, and
        // the kernel has to be able to reach the registers of the device.
        // The registers of the device are only written once the memory
        // space of the device is open.
        let command = device.command();
        device.set_command(
            command
                | crate::pci::COMMAND_MEMORY
                | crate::pci::COMMAND_BUS_MASTER
                | crate::pci::COMMAND_INTERRUPT_DISABLE,
        );

        // The size of the registers of the device is asked for before the
        // registers are used: the question overwrites the address of the
        // register for a moment.
        unsafe {
            crate::serial::write_fmt(core::format_args!(
                "Elvaris kernel: virtio: command before=0x{:04x} after=0x{:04x}\r\n",
                device.command(),
                device.command()
            ));
            device.write32(0x14, 0xFFFF_FFFF);
            crate::serial::write_fmt(core::format_args!(
                "Elvaris kernel: virtio: bar 1 after write=0x{:08x}\r\n",
                device.read32(0x14)
            ));
            device.write32(0x14, 0x81011000);
        }
        let mut sizes = [0u64; 6];
        for index in 0..6usize {
            sizes[index] = bar_size(device, index as u8);
        }

        let mut transport = Transport {
            common: core::ptr::null_mut(),
            device_config: core::ptr::null(),
            queue_count: 0,
            notify_capability: notify_cap,
            bars: [0; 6],
            device_base: DEVICE_MEMORY_BASE + slot as u64 * DEVICE_MEMORY_SLOT,
        };

        // Map the registers of the device. Every base address register gets
        // its own part of the memory of the kernel. A register of sixty-four
        // bits takes the register behind it, and the two are mapped together.
        let mut index = 0usize;
        while index < 6 {
            let kind = device.bar_kind(index as u8);
            let physical = device.bar_address(index as u8);
            unsafe {
                crate::serial::write_fmt(core::format_args!(
                    "Elvaris kernel: virtio: bar {} kind={:?} physical={:?} size=0x{:x}\r\n",
                    index, kind, physical, sizes[index]
                ));
            }
            match (kind, physical) {
                (crate::pci::BarKind::Memory64, Some(physical)) => {
                    let virtual_base = transport.device_base + index as u64 * DEVICE_MEMORY_STRIDE;
                    map_region(mapper, virtual_base, physical, sizes[index])?;
                    transport.bars[index] = virtual_base;
                    index += 2;
                }
                (crate::pci::BarKind::Memory, Some(physical)) => {
                    let virtual_base = transport.device_base + index as u64 * DEVICE_MEMORY_STRIDE;
                    map_region(mapper, virtual_base, physical, sizes[index])?;
                    transport.bars[index] = virtual_base;
                    index += 1;
                }
                _ => index += 1,
            }
        }

        let common_bar = transport.bar(common_cap.bar())?;
        transport.common = (common_bar + common_cap.offset_in_bar() as u64) as *mut u8;

        // The configuration of the device is optional: a device that has
        // none is read through its registers only.
        if let Some(device_cap) = capabilities
            .iter()
            .find(|capability| capability.kind() == CAP_DEVICE_CONFIG)
            .copied()
        {
            let device_bar = transport.bar(device_cap.bar())?;
            transport.device_config =
                (device_bar + device_cap.offset_in_bar() as u64) as *const u8;
        }

        // The features decide whether the device speaks the modern
        // interface at all.
        let features = transport.read_features();
        if features & FEATURE_VERSION_1 == 0 {
            return Err(VirtioError::NoModernInterface);
        }
        let accepted = FEATURE_VERSION_1 | (features & wanted);
        transport.write_features(accepted);
        transport.set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER);
        transport.set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK);
        if transport.status() & STATUS_FEATURES_OK == 0 {
            transport.set_status(STATUS_FAILED);
            return Err(VirtioError::FeaturesRefused);
        }
        transport.queue_count = transport.read16(OFFSET_QUEUE_COUNT);

        Ok(transport)
    }

    /// The base address of a base address register of the device.
    fn bar(&self, index: u8) -> Result<u64, VirtioError> {
        self.bars
            .get(index as usize)
            .copied()
            .filter(|base| *base != 0)
            .ok_or(VirtioError::NoCapabilities)
    }

    /// Read a byte of the common configuration.
    fn read8(&self, offset: usize) -> u8 {
        // SAFETY: the common configuration is mapped into the memory of
        // the kernel, and the offset is inside the structure.
        unsafe { core::ptr::read_volatile(self.common.add(offset)) }
    }

    /// Read two bytes of the common configuration.
    fn read16(&self, offset: usize) -> u16 {
        // SAFETY: the common configuration is mapped into the memory of
        // the kernel, and the offset is inside the structure.
        unsafe { core::ptr::read_volatile(self.common.add(offset) as *const u16) }
    }

    /// Read four bytes of the common configuration.
    fn read32(&self, offset: usize) -> u32 {
        // SAFETY: the common configuration is mapped into the memory of
        // the kernel, and the offset is inside the structure.
        unsafe { core::ptr::read_volatile(self.common.add(offset) as *const u32) }
    }

    /// Write two bytes of the common configuration.
    fn write16(&self, offset: usize, value: u16) {
        // SAFETY: the common configuration is mapped into the memory of
        // the kernel, and the offset is inside the structure.
        unsafe { core::ptr::write_volatile(self.common.add(offset) as *mut u16, value) }
    }

    /// Write four bytes of the common configuration.
    fn write32(&self, offset: usize, value: u32) {
        // SAFETY: the common configuration is mapped into the memory of
        // the kernel, and the offset is inside the structure.
        unsafe { core::ptr::write_volatile(self.common.add(offset) as *mut u32, value) }
    }

    /// Write eight bytes of the common configuration.
    fn write64(&self, offset: usize, value: u64) {
        // SAFETY: the common configuration is mapped into the memory of
        // the kernel, and the offset is inside the structure.
        unsafe { core::ptr::write_volatile(self.common.add(offset) as *mut u64, value) }
    }

    /// The status of the device.
    fn status(&self) -> u8 {
        self.read8(OFFSET_STATUS)
    }

    /// Change the status of the device.
    fn set_status(&self, status: u8) {
        self.write32(OFFSET_STATUS, status as u32);
    }

    /// The feature bits the device offers.
    fn read_features(&self) -> u64 {
        let mut features = 0u64;
        for select in 0..2u32 {
            self.write32(OFFSET_FEATURE_SELECT, select);
            let half = self.read32(OFFSET_DEVICE_FEATURE) as u64;
            features |= half << (select * 32);
        }
        features
    }

    /// Tell the device which feature bits the driver understands.
    fn write_features(&self, features: u64) {
        for select in 0..2u32 {
            self.write32(OFFSET_DRIVER_FEATURE_SELECT, select);
            let half = (features >> (select * 32)) as u32;
            self.write32(OFFSET_DRIVER_FEATURE, half);
        }
    }

    /// The number of virtqueues of the device.
    pub fn queue_count(&self) -> u16 {
        self.queue_count
    }

    /// The size of a virtqueue of the device.
    ///
    /// A device that has no such virtqueue answers with zero.
    pub fn queue_size(&self, index: u16) -> u16 {
        self.write16(OFFSET_QUEUE_SELECT, index);
        self.read16(OFFSET_QUEUE_SIZE)
    }

    /// Give the device the addresses of the three parts of a virtqueue and
    /// enable the queue.
    ///
    /// The memory of the queue has to be physically contiguous, and it has
    /// to stay where it is while the queue is used. The function returns
    /// the address of the register that tells the device that a request is
    /// available.
    pub fn setup_queue(
        &self,
        index: u16,
        descriptor: u64,
        available: u64,
        used: u64,
    ) -> Result<u64, VirtioError> {
        self.write16(OFFSET_QUEUE_SELECT, index);
        self.write64(OFFSET_QUEUE_DESC, descriptor);
        self.write64(OFFSET_QUEUE_DRIVER, available);
        self.write64(OFFSET_QUEUE_DEVICE, used);
        self.write16(OFFSET_QUEUE_VECTOR, 0xFFFF);
        self.write16(OFFSET_QUEUE_ENABLE, 1);

        // The notification register of the queue lies in the register of
        // the notifications, at the offset the device names.
        let notify_offset = u64::from(self.read16(OFFSET_QUEUE_NOTIFY));
        Ok(self.notify_base()? + notify_offset * self.notify_multiplier() as u64)
    }

    /// Tell the device that the driver is ready for the requests.
    pub fn set_ready(&self) {
        self.set_status(STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK);
    }

    /// The address of the register of the notifications.
    fn notify_base(&self) -> Result<u64, VirtioError> {
        let capability = self.notify_capability;
        let bar = self.bar(capability.bar())?;
        Ok(bar + capability.offset_in_bar() as u64)
    }

    /// The multiplier of the offset of the notification register.
    fn notify_multiplier(&self) -> u32 {
        self.notify_capability.notify_multiplier()
    }

    /// Read a value of the configuration of the device.
    ///
    /// The function returns `None` when the device has no configuration.
    pub fn device_config_u64(&self, offset: usize) -> Option<u64> {
        if self.device_config.is_null() {
            return None;
        }
        // SAFETY: the configuration of the device is mapped into the memory
        // of the kernel, and the offset is inside the structure.
        Some(unsafe { core::ptr::read_volatile(self.device_config.add(offset) as *const u64) })
    }
}

/// A virtqueue of a virtio device.
pub struct Queue {
    /// The physical address of the descriptor table.
    descriptors: u64,
    /// The physical address of the available ring.
    available: u64,
    /// The physical address of the used ring.
    used: u64,
    /// The virtual address of the register that tells the device that a
    /// request is available.
    notify: u64,
    /// The number of the queue.
    index: u16,
    /// The number of descriptors of the queue.
    size: u16,
    /// The number of the next descriptor the driver offers.
    next_available: AtomicU16,
    /// The number of the last descriptor the device has finished.
    last_used: AtomicU16,
}

impl Queue {
    /// Build a queue over the memory of the kernel.
    ///
    /// The three parts of the queue are addressed by their physical
    /// addresses, because the device reaches them directly.
    pub fn new(
        index: u16,
        descriptors: u64,
        available: u64,
        used: u64,
        notify: u64,
        size: u16,
    ) -> Self {
        Queue {
            descriptors,
            available,
            used,
            notify,
            index,
            size,
            next_available: AtomicU16::new(0),
            last_used: AtomicU16::new(0),
        }
    }

    /// The number of descriptors of the queue.
    pub fn size(&self) -> u16 {
        self.size
    }

    /// The number of bytes the descriptor table of a queue of that size
    /// needs.
    pub fn descriptor_bytes(size: u16) -> u64 {
        DESCRIPTOR_SIZE * size as u64
    }

    /// The number of bytes the available ring of a queue of that size needs.
    pub fn available_bytes(size: u16) -> u64 {
        AVAIL_HEADER_SIZE + 2 * size as u64
    }

    /// The number of bytes the used ring of a queue of that size needs.
    pub fn used_bytes(size: u16) -> u64 {
        USED_HEADER_SIZE + USED_ENTRY_SIZE * size as u64
    }

    /// Write a descriptor of the table.
    fn write_descriptor(&self, index: u16, address: u64, length: u32, flags: u16, next: u16) {
        let offset = self.descriptors + index as u64 * DESCRIPTOR_SIZE;
        // SAFETY: the descriptor table is in memory the device reads, and
        // the descriptor is inside the table.
        unsafe {
            core::ptr::write_volatile(offset as *mut u64, address);
            core::ptr::write_volatile((offset + 8) as *mut u32, length);
            core::ptr::write_volatile((offset + 12) as *mut u16, flags);
            core::ptr::write_volatile((offset + 14) as *mut u16, next);
        }
    }

    /// The number the used ring holds.
    fn used_index(&self) -> u16 {
        // SAFETY: the used ring is in memory the device writes, and the
        // index is inside the ring.
        unsafe { core::ptr::read_volatile((self.used + 2) as *const u16) }
    }

    /// Make a request available to the device.
    ///
    /// The request is made of three descriptors: the header of the request,
    /// the data of the request, and the status the device writes. The
    /// function returns the number of bytes the device has written.
    pub fn submit(
        &self,
        header: u64,
        data: u64,
        data_length: u32,
        device_writes_data: bool,
        status: u64,
    ) -> Result<u32, VirtioError> {
        let index = self.next_available.load(Ordering::Relaxed) % self.size;
        // The driver waits for the device before it offers the next
        // request, so only one request is on its way at a time. The three
        // descriptors of the request are the first three of the table.
        self.write_descriptor(0, header, 16, DESCRIPTOR_NEXT, 1);
        let data_flags = DESCRIPTOR_NEXT | if device_writes_data { DESCRIPTOR_WRITE } else { 0 };
        self.write_descriptor(1, data, data_length, data_flags, 2);
        self.write_descriptor(2, status, 1, DESCRIPTOR_WRITE, 0);

        // The descriptor has to be in place before the device is told about
        // it.
        fence(Ordering::SeqCst);

        // SAFETY: the available ring is in memory the device reads, and the
        // entry is inside the ring.
        unsafe {
            core::ptr::write_volatile(
                (self.available + AVAIL_HEADER_SIZE + index as u64 * 2) as *mut u16,
                0,
            );
        }
        let next = self.next_available.load(Ordering::Relaxed).wrapping_add(1);
        self.next_available.store(next, Ordering::Relaxed);
        // SAFETY: the index of the available ring is in memory the device
        // reads.
        unsafe {
            core::ptr::write_volatile((self.available + 2) as *mut u16, next);
        }

        fence(Ordering::SeqCst);

        // Tell the device that the request is available.
        // SAFETY: the notification register is mapped into the memory of
        // the kernel, and the address is inside the region of the device.
        unsafe {
            core::ptr::write_volatile(self.notify as *mut u16, self.index);
        }

        // Wait for the device to finish the request.
        let mut spins = 0u64;
        while self.used_index() == self.last_used.load(Ordering::Relaxed) {
            spins += 1;
            if spins > COMPLETION_SPINS {
                return Err(VirtioError::Timeout);
            }
            core::hint::spin_loop();
        }
        fence(Ordering::SeqCst);

        // The entry of the used ring names the descriptor the device has
        // finished and the number of bytes it has written.
        let last_used = self.last_used.load(Ordering::Relaxed);
        let entry = self.used
            + USED_HEADER_SIZE
            + (last_used % self.size) as u64 * USED_ENTRY_SIZE;
        // SAFETY: the used ring is in memory the device writes, and the
        // entry is inside the ring.
        let length = unsafe { core::ptr::read_volatile(entry as *const u32) };
        self.last_used.store(last_used.wrapping_add(1), Ordering::Relaxed);
        Ok(length)
    }
}

/// Round a number up to the next multiple of the alignment.
fn align_up(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}

/// The addresses of the three parts of a virtqueue.
///
/// The three parts lie one behind the other in the memory that was given
/// to the queue. Each of them starts at an address that suits it: sixteen
/// bytes for the descriptor table, two for the available ring, and four for
/// the used ring.
pub fn queue_layout(memory: u64, size: u16) -> (u64, u64, u64) {
    let descriptor = align_up(memory, 16);
    let available = align_up(descriptor + Queue::descriptor_bytes(size), 2);
    let used = align_up(available + Queue::available_bytes(size), 4);
    (descriptor, available, used)
}

/// Map a region of the memory of a device into the memory of the kernel.
///
/// The registers of a device must not be cached: the device changes them
/// without the processor knowing, and the processor must not change them
/// without the device knowing.
fn map_region(
    mapper: &mut Mapper,
    virtual_base: u64,
    physical: u64,
    size: u64,
) -> Result<(), VirtioError> {
    let pages = size.div_ceil(crate::vmm::page_table::PAGE_SIZE);
    for index in 0..pages {
        let address = physical + index * crate::vmm::page_table::PAGE_SIZE;
        let target = virtual_base + index * crate::vmm::page_table::PAGE_SIZE;
        mapper
            .map_page_to(target, address, crate::cpu::MMIO_UNCACHED_FLAGS)
            .map_err(VirtioError::MappingFailed)?;
    }
    Ok(())
}

/// The size of a base address register of a device.
///
/// The size of a register is not written anywhere: the kernel writes ones
/// to the register and reads back how many of them stayed. The bits that
/// stayed are the bits of the address, and the size is what is left of
/// them. The function returns the size of the register in bytes, or zero
/// when the register cannot be used.
fn bar_size(device: &Device, index: u8) -> u64 {
    let offset = 0x10 + index as u16 * 4;
    let original = device.read32(offset);
    if original & 1 != 0 {
        // The register describes the input and output space: its size is
        // not known this way.
        return 0;
    }
    // A register of sixty-four bits takes the register behind it for its
    // upper half, and the size is written into both halves.
    let is_64_bit = device.bar_kind(index) == crate::pci::BarKind::Memory64;
    let original_high = if is_64_bit {
        device.read32(offset + 4)
    } else {
        0
    };

    if is_64_bit {
        device.write32(offset, 0xFFFF_FFFF);
        device.write32(offset + 4, 0xFFFF_FFFF);
    } else {
        device.write32(offset, 0xFFFF_FFFF);
    }

    let low = device.read32(offset);
    let mut mask = (low & 0xFFFF_FFF0) as u64;
    if is_64_bit {
        // The upper half of the register holds address bits and nothing
        // else, so all of them count.
        let high = device.read32(offset + 4);
        mask |= (high as u64) << 32;
    }

    // The address of the register has to be put back.
    if is_64_bit {
        device.write32(offset, original);
        device.write32(offset + 4, original_high);
    } else {
        device.write32(offset, original);
    }

    // The size is what is left of the address: for a register of
    // thirty-two bits the address is thirty-two bits long as well.
    let size = if is_64_bit {
        (!mask).wrapping_add(1)
    } else {
        ((!mask & 0xFFFF_FFFF) + 1) as u64
    };
    if size == 0 || size > 0x0010_0000 {
        return 0;
    }
    size
}
