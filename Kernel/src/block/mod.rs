//! The block-device interface.
//!
//! A block device is the lowest layer of the storage stack: it moves blocks
//! of a fixed size between the kernel and a medium. The filesystem above it
//! never talks to the medium directly; it only asks a block device to read
//! and write blocks.
//!
//! # The interface
//!
//! The interface is deliberately small, because every method has to be
//! implementable by a driver that talks to real hardware:
//!
//! | Method | Purpose |
//! |--------|---------|
//! | [`BlockDevice::block_size`] | The size of one block in bytes. Every buffer handed to the device has to be exactly this long. |
//! | [`BlockDevice::block_count`] | The number of blocks the device holds. |
//! | [`BlockDevice::capacity_bytes`] | The size of the whole device in bytes. |
//! | [`BlockDevice::device_id`] | A short name that identifies the device in the diagnostics. |
//! | [`BlockDevice::is_ready`] | Whether the device can be used at all. |
//! | [`BlockDevice::read_block`] | Read one block into a buffer. |
//! | [`BlockDevice::write_block`] | Write one block from a buffer. |
//! | [`BlockDevice::flush`] | Make the writes durable. A device that has no durable medium does nothing. |
//!
//! # Rules
//!
//! - A buffer handed to a read or a write must be exactly [`BLOCK_SIZE`]
//!   bytes long. A buffer of a different length is refused with
//!   [`BlockError::BadBufferSize`] instead of being truncated or padded.
//! - A block index outside the device is refused with
//!   [`BlockError::OutOfRange`].
//! - The device never panics and never writes out of bounds: every failure
//!   is reported as an error.
//!
//! # Limitations
//!
//! - There is no request queue and no interrupt handling: the transfers are
//!   synchronous. A driver for a real disk has to wait for the transfer to
//!   finish, and the kernel is blocked while it waits.
//! - There is no partitioning, no sector size negotiation and no error
//!   correction. The block size is a property of the device, not something
//!   the caller can choose.

use alloc::vec::Vec;

#[cfg(test)]
mod tests;

pub mod virtio;

/// The size of one block in bytes.
///
/// The value is fixed for the whole milestone: every block device the
/// kernel talks to uses the same block size, so the filesystem above does
/// not have to care about the medium.
pub const BLOCK_SIZE: usize = 512;

/// The errors a block device reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    /// The block index is outside the device.
    OutOfRange,
    /// The buffer is not exactly one block long.
    BadBufferSize,
    /// The transfer failed.
    IoError,
    /// The device is not usable at the moment.
    NotReady,
    /// The device is busy with another transfer.
    DeviceBusy,
}

impl BlockError {
    /// A short description of the error, used in the serial diagnostics.
    pub const fn description(&self) -> &'static str {
        match self {
            BlockError::OutOfRange => "the block index is outside the device",
            BlockError::BadBufferSize => "the buffer is not one block long",
            BlockError::IoError => "the transfer failed",
            BlockError::NotReady => "the device is not ready",
            BlockError::DeviceBusy => "the device is busy",
        }
    }
}

/// A device that stores blocks of a fixed size.
///
/// The trait is object safe on purpose: the filesystem holds a reference to
/// the device it was mounted on, and the kernel can therefore choose the
/// medium at run time.
///
/// The trait asks for `Send` and `Sync`: the filesystem of the machine lives
/// in a static variable, and the static variable has to be safe to reach
/// from every part of the kernel.
pub trait BlockDevice: Send + Sync {
    /// The size of one block in bytes.
    fn block_size(&self) -> usize;

    /// The number of blocks the device holds.
    fn block_count(&self) -> usize;

    /// A short name that identifies the device in the diagnostics.
    fn device_id(&self) -> &str;

    /// Whether the device can be used at the moment.
    ///
    /// A device that is not ready refuses every transfer with
    /// [`BlockError::NotReady`].
    fn is_ready(&self) -> bool;

    /// Read one block into `buffer`.
    ///
    /// The buffer has to be exactly [`BLOCK_SIZE`] bytes long. The device
    /// fills the whole buffer; it never leaves part of it undefined.
    fn read_block(&self, index: usize, buffer: &mut [u8]) -> Result<(), BlockError>;

    /// Write one block from `buffer`.
    ///
    /// The buffer has to be exactly [`BLOCK_SIZE`] bytes long.
    fn write_block(&mut self, index: usize, buffer: &[u8]) -> Result<(), BlockError>;

    /// Make the writes durable.
    ///
    /// A device whose medium loses the data when the machine stops does
    /// nothing here and reports success. The default implementation is
    /// exactly that.
    fn flush(&mut self) -> Result<(), BlockError> {
        Ok(())
    }

    /// The size of the whole device in bytes.
    fn capacity_bytes(&self) -> usize {
        self.block_size().saturating_mul(self.block_count())
    }

    /// Read several consecutive blocks into one buffer.
    ///
    /// The buffer has to be a whole number of blocks long. The blocks are
    /// read one after the other, and the first failure ends the transfer.
    /// The blocks that were already read keep their content.
    fn read_blocks(&self, index: usize, buffer: &mut [u8]) -> Result<(), BlockError> {
        if buffer.len() % BLOCK_SIZE != 0 {
            return Err(BlockError::BadBufferSize);
        }
        let count = buffer.len() / BLOCK_SIZE;
        if index.saturating_add(count) > self.block_count() {
            return Err(BlockError::OutOfRange);
        }
        for offset in 0..count {
            let start = offset * BLOCK_SIZE;
            self.read_block(index + offset, &mut buffer[start..start + BLOCK_SIZE])?;
        }
        Ok(())
    }

    /// Write several consecutive blocks from one buffer.
    ///
    /// The buffer has to be a whole number of blocks long. The first failure
    /// ends the transfer; the blocks that were already written keep their
    /// content.
    fn write_blocks(&mut self, index: usize, buffer: &[u8]) -> Result<(), BlockError> {
        if buffer.len() % BLOCK_SIZE != 0 {
            return Err(BlockError::BadBufferSize);
        }
        let count = buffer.len() / BLOCK_SIZE;
        if index.saturating_add(count) > self.block_count() {
            return Err(BlockError::OutOfRange);
        }
        for offset in 0..count {
            let start = offset * BLOCK_SIZE;
            self.write_block(index + offset, &buffer[start..start + BLOCK_SIZE])?;
        }
        Ok(())
    }
}

/// A block device that keeps its blocks in memory.
///
/// The device is the substrate of the in-memory filesystem: the filesystem
/// writes its blocks into the device, and it reads them back from it. It is
/// also the substrate of every future on-disk format, because the format
/// only changes which bytes are written where.
///
/// # Limitations
///
/// - The blocks live in the kernel heap. A device of `n` blocks needs
///   `n * 512` bytes of heap memory, and the heap is not grown on demand.
/// - The device forgets everything when the machine stops. There is no
///   durable medium behind it.
pub struct MemoryBlockDevice {
    blocks: Vec<u8>,
    count: usize,
    ready: bool,
}

impl MemoryBlockDevice {
    /// Create a device with `count` blocks, all of them zero.
    ///
    /// A device with zero blocks is valid: it is an empty medium, and every
    /// access to it is refused with [`BlockError::OutOfRange`].
    pub fn new(count: usize) -> Self {
        Self {
            blocks: alloc::vec![0u8; count * BLOCK_SIZE],
            count,
            ready: true,
        }
    }

    /// Create a device that holds the given bytes.
    ///
    /// The data is padded with zeros to a whole number of blocks. A data
    /// block that is longer than the device is refused with
    /// [`BlockError::OutOfRange`].
    pub fn from_bytes(data: &[u8]) -> Result<Self, BlockError> {
        let count = data.len().div_ceil(BLOCK_SIZE);
        let mut device = Self::new(count);
        device.blocks[..data.len()].copy_from_slice(data);
        Ok(device)
    }

    /// The number of blocks the device holds.
    pub fn block_count(&self) -> usize {
        self.count
    }

    /// The contents of the whole device as one byte vector.
    ///
    /// The function is used by the tests and by the on-disk format code; it
    /// copies the blocks, so the caller cannot reach into the device.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.blocks.clone()
    }

    /// Mark the device as unusable.
    ///
    /// A device that is not ready refuses every transfer. The kernel uses
    /// this when a medium fails.
    pub fn set_ready(&mut self, ready: bool) {
        self.ready = ready;
    }
}

impl BlockDevice for MemoryBlockDevice {
    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    fn block_count(&self) -> usize {
        self.count
    }

    fn device_id(&self) -> &str {
        "memory"
    }

    fn is_ready(&self) -> bool {
        self.ready
    }

    fn read_block(&self, index: usize, buffer: &mut [u8]) -> Result<(), BlockError> {
        if !self.ready {
            return Err(BlockError::NotReady);
        }
        if index >= self.count {
            return Err(BlockError::OutOfRange);
        }
        if buffer.len() != BLOCK_SIZE {
            return Err(BlockError::BadBufferSize);
        }
        let offset = index * BLOCK_SIZE;
        buffer.copy_from_slice(&self.blocks[offset..offset + BLOCK_SIZE]);
        Ok(())
    }

    fn write_block(&mut self, index: usize, buffer: &[u8]) -> Result<(), BlockError> {
        if !self.ready {
            return Err(BlockError::NotReady);
        }
        if index >= self.count {
            return Err(BlockError::OutOfRange);
        }
        if buffer.len() != BLOCK_SIZE {
            return Err(BlockError::BadBufferSize);
        }
        let offset = index * BLOCK_SIZE;
        self.blocks[offset..offset + BLOCK_SIZE].copy_from_slice(buffer);
        Ok(())
    }
}
