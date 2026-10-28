//! Packet buffer management.

use alloc::vec::Vec;

pub const MAX_PACKET_SIZE: usize = 1518;
pub const PACKET_HEADROOM: usize = 16;
pub const MAX_RX_QUEUE: usize = 256;
pub const MAX_TX_QUEUE: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketError {
    TooLarge,
    OutOfBounds,
    InvalidLength,
    UseAfterRelease,
    DoubleRelease,
    NoSpace,
    QueueFull,
}

#[derive(Debug)]
pub struct PacketBuffer {
    data: Vec<u8>,
    len: usize,
    headroom: usize,
    tailroom: usize,
    owned: bool,
}

impl PacketBuffer {
    pub fn new() -> Self {
        let mut data = Vec::new();
        data.resize(MAX_PACKET_SIZE + PACKET_HEADROOM, 0);
        PacketBuffer {
            data,
            len: 0,
            headroom: PACKET_HEADROOM,
            tailroom: MAX_PACKET_SIZE,
            owned: true,
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        let mut data = Vec::new();
        data.resize(cap + PACKET_HEADROOM, 0);
        PacketBuffer {
            data,
            len: 0,
            headroom: PACKET_HEADROOM,
            tailroom: cap,
            owned: true,
        }
    }

    pub fn capacity(&self) -> usize {
        if self.data.len() >= self.headroom { self.data.len() - self.headroom } else { 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<(), PacketError> {
        if !self.owned {
            return Err(PacketError::UseAfterRelease);
        }
        if bytes.len() > self.capacity() {
            return Err(PacketError::TooLarge);
        }
        if self.len + bytes.len() > self.capacity() {
            return Err(PacketError::TooLarge);
        }
        let start = self.headroom;
        self.data[start + self.len..start + self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }

    pub fn data(&self) -> &[u8] {
        if self.len == 0 {
            &[]
        } else {
            &self.data[self.headroom..self.headroom + self.len]
        }
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        let start = self.headroom;
        &mut self.data[start..start + self.len]
    }

    pub fn take(&mut self) -> Option<PacketBuffer> {
        if !self.owned {
            return None;
        }
        self.owned = false;
        Some(PacketBuffer {
            data: self.data.clone(),
            len: self.len,
            headroom: self.headroom,
            tailroom: self.tailroom,
            owned: true,
        })
    }

    pub fn release(&mut self) -> Result<(), PacketError> {
        if !self.owned {
            return Err(PacketError::DoubleRelease);
        }
        self.len = 0;
        self.owned = false;
        Ok(())
    }
}

impl Default for PacketBuffer {
    fn default() -> Self {
        Self::new()
    }
}

