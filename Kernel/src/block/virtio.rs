//! The virtio block device.
//!
//! The device is a disk that lives in the host: the kernel asks the host to
//! move the data of a block, and the host does it. The device speaks the
//! modern virtio interface (see [`crate::virtio`]), and every request is
//! made of three parts: the header of the request, the data of the request,
//! and the status the host writes when the request is done.
//!
//! # The limits
//!
//! - The requests move one block at a time.
//! - The device does not use interrupts: the kernel waits for the used ring
//!   of the virtqueue instead, and the device is told not to raise an
//!   interrupt.

use crate::block::{BLOCK_SIZE, BlockDevice, BlockError};
use crate::pci;
use crate::virtio::{Queue, Transport, VirtioError, queue_layout};
use crate::vmm::mapper::Mapper;
use crate::vmm::page_table::PAGE_SIZE;

/// The kind of a request that reads a block.
const REQUEST_READ: u32 = 0;

/// The kind of a request that writes a block.
const REQUEST_WRITE: u32 = 1;

/// The kind of a request that flushes the device.
const REQUEST_FLUSH: u32 = 4;

/// The feature bit that says that the device can flush its medium.
const FEATURE_FLUSH: u64 = 1 << 9;

/// The offset of the header of a request inside the buffer of the request.
const HEADER_OFFSET: u64 = 0;

/// The offset of the data of a request inside the buffer of the request.
const DATA_OFFSET: u64 = 512;

/// The offset of the status of a request inside the buffer of the request.
const STATUS_OFFSET: u64 = 1024;

/// The status the host writes when the request went well.
const STATUS_OK: u8 = 0;

/// A disk that lives in the host.
pub struct VirtioBlockDevice {
    /// The virtqueue the requests travel through.
    queue: Queue,
    /// The number of blocks the device holds.
    blocks: usize,
    /// The physical address of the buffer of the requests.
    request: u64,
}

impl VirtioBlockDevice {
    /// Open the disk of a device of the PCI bus.
    ///
    /// The function prepares the device of the bus, builds the virtqueue the
    /// requests travel through, and reads the capacity of the disk. The
    /// `slot` tells the driver which part of the memory of the kernel the
    /// registers of the device are mapped into, so that two devices do not
    /// fight over the same place.
    pub fn open(
        device: &pci::Device,
        mapper: &mut Mapper,
        slot: usize,
    ) -> Result<Self, VirtioError> {
        let transport = Transport::new(device, mapper, FEATURE_FLUSH, slot)?;

        // The capacity of the disk is the number of blocks it holds.
        let capacity = transport
            .device_config_u64(0)
            .ok_or(VirtioError::NoCapabilities)?;

        // The requests travel through the first virtqueue of the device.
        let size = transport.queue_size(0);
        if size == 0 {
            return Err(VirtioError::NoQueue);
        }

        // The memory of the virtqueue has to be physically contiguous: the
        // host reaches the three parts of the queue directly.
        let bytes = Queue::descriptor_bytes(size)
            + Queue::available_bytes(size)
            + Queue::used_bytes(size);
        let pages = bytes.div_ceil(PAGE_SIZE);
        let memory = mapper
            .allocate_frames(pages as usize)
            .map_err(VirtioError::MappingFailed)?;
        let (descriptor, available, used) = queue_layout(memory, size);
        let notify = transport.setup_queue(0, descriptor, available, used)?;

        // The buffer of the requests holds the header, the data, and the
        // status of the request. It fits in one page.
        let request = mapper
            .allocate_frames(1)
            .map_err(VirtioError::MappingFailed)?;

        transport.set_ready();

        Ok(VirtioBlockDevice {
            queue: Queue::new(0, descriptor, available, used, notify, size),
            blocks: capacity as usize,
            request,
        })
    }

    /// The virtual address of the buffer of the requests.
    fn buffer(&self) -> u64 {
        self.request + crate::vmm::physical_memory_offset()
    }

    /// Write the header of a request into the buffer of the requests.
    fn write_header(&self, kind: u32, block: u64) {
        let buffer = self.buffer();
        // SAFETY: the buffer of the requests is in memory the host reads,
        // and the header is inside the buffer.
        unsafe {
            let header = (buffer + HEADER_OFFSET) as *mut u32;
            header.write_volatile(kind);
            header.add(1).write_volatile(0);
            (header.add(2) as *mut u64).write_volatile(block);
            ((buffer + STATUS_OFFSET) as *mut u8).write_volatile(0xFF);
        }
    }

    /// The status the host wrote for the last request.
    fn status(&self) -> u8 {
        // SAFETY: the status of the request is written by the host, and it
        // is inside the buffer of the requests.
        unsafe { core::ptr::read_volatile((self.buffer() + STATUS_OFFSET) as *const u8) }
    }

    /// Move one block of the disk.
    ///
    /// The function reads the block when `read` is true, and writes it
    /// otherwise. The data of the request is in the buffer of the requests
    /// while the host works on it.
    fn transfer(&self, block: usize, data: &mut [u8], read: bool) -> Result<(), BlockError> {
        if data.len() != BLOCK_SIZE {
            return Err(BlockError::BadBufferSize);
        }
        let buffer = self.buffer();

        self.write_header(if read { REQUEST_READ } else { REQUEST_WRITE }, block as u64);
        // SAFETY: the buffer of the requests is in memory the host reads.
        unsafe {
            // The data of the request is copied into the buffer before the
            // request is made, and copied out of it when the request is
            // done.
            if !read {
                core::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    (buffer + DATA_OFFSET) as *mut u8,
                    data.len(),
                );
            }
        }

        self.queue
            .submit(
                self.request + HEADER_OFFSET,
                self.request + DATA_OFFSET,
                data.len() as u32,
                read,
                self.request + STATUS_OFFSET,
            )
            .map_err(|_| BlockError::IoError)?;

        // SAFETY: the status of the request is written by the host.
        if self.status() != STATUS_OK {
            return Err(BlockError::IoError);
        }

        if read {
            // SAFETY: the host has written the data of the block into the
            // buffer of the requests, and the buffer is longer than the
            // data.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    (buffer + DATA_OFFSET) as *const u8,
                    data.as_mut_ptr(),
                    data.len(),
                );
            }
        }

        Ok(())
    }

    /// Ask the host to write everything it has into the medium.
    fn flush_device(&mut self) -> Result<(), BlockError> {
        self.write_header(REQUEST_FLUSH, 0);

        self.queue
            .submit(
                self.request + HEADER_OFFSET,
                self.request + DATA_OFFSET,
                0,
                false,
                self.request + STATUS_OFFSET,
            )
            .map_err(|_| BlockError::IoError)?;

        if self.status() != STATUS_OK {
            return Err(BlockError::IoError);
        }
        Ok(())
    }
}

impl BlockDevice for VirtioBlockDevice {
    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    fn block_count(&self) -> usize {
        self.blocks
    }

    fn device_id(&self) -> &str {
        "virtio-blk"
    }

    fn is_ready(&self) -> bool {
        true
    }

    fn read_block(&self, index: usize, buffer: &mut [u8]) -> Result<(), BlockError> {
        if index >= self.blocks {
            return Err(BlockError::OutOfRange);
        }
        self.transfer(index, buffer, true)
    }

    fn write_block(&mut self, index: usize, buffer: &[u8]) -> Result<(), BlockError> {
        if index >= self.blocks {
            return Err(BlockError::OutOfRange);
        }
        if buffer.len() != BLOCK_SIZE {
            return Err(BlockError::BadBufferSize);
        }
        let mut data = [0u8; BLOCK_SIZE];
        data.copy_from_slice(buffer);
        self.transfer(index, &mut data, false)
    }

    fn flush(&mut self) -> Result<(), BlockError> {
        self.flush_device()
    }
}
