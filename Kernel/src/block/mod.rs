//! Block device abstraction.

#![allow(dead_code)]

pub const BLOCK_SIZE: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    OutOfRange,
    BadBufferSize,
    IoError,
}

pub trait BlockDevice {
    fn block_size(&self) -> usize;
    fn block_count(&self) -> usize;
    fn read_block(&self, index: usize, buf: &mut [u8]) -> Result<(), BlockError>;
    fn write_block(&mut self, index: usize, buf: &[u8]) -> Result<(), BlockError>;
}

pub struct MemoryBlockDevice {
    blocks: Vec<u8>,
    count: usize,
}

impl MemoryBlockDevice {
    pub fn new(count: usize) -> Self {
        Self {
            blocks: vec![0u8; count * BLOCK_SIZE],
            count,
        }
    }
}

impl BlockDevice for MemoryBlockDevice {
    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }
    fn block_count(&self) -> usize {
        self.count
    }
    fn read_block(&self, index: usize, buf: &mut [u8]) -> Result<(), BlockError> {
        if index >= self.count {
            return Err(BlockError::OutOfRange);
        }
        if buf.len() != BLOCK_SIZE {
            return Err(BlockError::BadBufferSize);
        }
        let off = index * BLOCK_SIZE;
        buf.copy_from_slice(&self.blocks[off..off + BLOCK_SIZE]);
        Ok(())
    }
    fn write_block(&mut self, index: usize, buf: &[u8]) -> Result<(), BlockError> {
        if index >= self.count {
            return Err(BlockError::OutOfRange);
        }
        if buf.len() != BLOCK_SIZE {
            return Err(BlockError::BadBufferSize);
        }
        let off = index * BLOCK_SIZE;
        self.blocks[off..off + BLOCK_SIZE].copy_from_slice(buf);
        Ok(())
    }
}
