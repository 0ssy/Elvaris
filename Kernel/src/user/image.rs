//! The format of a program image.
//!
//! A program image is the file the kernel loads a program from. The format
//! is deliberately simple: a fixed header and a block of machine code. There
//! is no relocation, no symbol table and no dynamic linking, so the image is
//! only usable at the address the loader puts it at.
//!
//! # The layout
//!
//! ```text
//!  offset  size  field
//!  0       4     magic, "ELVI"
//!  4       4     the version of the format
//!  8       4     the offset of the entry point inside the payload
//!  12      4     the size of the payload in bytes
//!  16      ...   the payload: the machine code of the program
//! ```
//!
//! The payload is loaded at the base of the user code page, so the entry
//! point of the program is the base of the code page plus the offset of the
//! entry point.
//!
//! # The rules
//!
//! - The magic has to be exactly `ELVI`.
//! - The version has to be the version this kernel understands.
//! - The size of the payload may not exceed the size of the file.
//! - The offset of the entry point has to point inside the payload.
//! - The payload may not be larger than the user code page.
//!
//! An image that breaks one of the rules is refused. The kernel never
//! loads part of an image and never runs an image whose entry point it
//! could not check.

use alloc::vec::Vec;

/// The magic that starts every program image.
pub const IMAGE_MAGIC: [u8; 4] = *b"ELVI";

/// The version of the image format this kernel reads.
pub const IMAGE_VERSION: u32 = 1;

/// The size of the header of a program image.
pub const IMAGE_HEADER_SIZE: usize = 16;

/// The reasons a program image is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageError {
    /// The file is too short to hold a header.
    TooShort,
    /// The file does not start with the magic.
    BadMagic,
    /// The version of the format is not the one this kernel understands.
    UnsupportedVersion,
    /// The size of the payload does not fit into the file.
    PayloadTooLarge,
    /// The entry point does not point inside the payload.
    EntryOutOfRange,
}

impl ImageError {
    /// A short description of the error, used in the serial diagnostics.
    pub const fn description(&self) -> &'static str {
        match self {
            ImageError::TooShort => "the image is too short",
            ImageError::BadMagic => "the image does not start with the magic",
            ImageError::UnsupportedVersion => "the version of the image is not supported",
            ImageError::PayloadTooLarge => "the payload does not fit into the image",
            ImageError::EntryOutOfRange => "the entry point is outside the payload",
        }
    }
}

/// The header of a program image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageHeader {
    /// The version of the format.
    pub version: u32,
    /// The offset of the entry point inside the payload.
    pub entry_offset: u32,
    /// The size of the payload in bytes.
    pub payload_size: u32,
}

impl ImageHeader {
    /// Read and check the header of an image.
    ///
    /// The function never looks at the payload itself: it only checks that
    /// the header is consistent with the size of the file.
    pub fn parse(bytes: &[u8]) -> Result<Self, ImageError> {
        if bytes.len() < IMAGE_HEADER_SIZE {
            return Err(ImageError::TooShort);
        }
        if bytes[0..4] != IMAGE_MAGIC {
            return Err(ImageError::BadMagic);
        }
        let version = read_u32(bytes, 4);
        if version != IMAGE_VERSION {
            return Err(ImageError::UnsupportedVersion);
        }
        let entry_offset = read_u32(bytes, 8);
        let payload_size = read_u32(bytes, 12);

        let available = bytes.len() - IMAGE_HEADER_SIZE;
        if payload_size as usize > available {
            return Err(ImageError::PayloadTooLarge);
        }
        if entry_offset as usize >= payload_size as usize {
            return Err(ImageError::EntryOutOfRange);
        }
        Ok(ImageHeader {
            version,
            entry_offset,
            payload_size,
        })
    }

    /// The payload of the image.
    pub fn payload<'a>(&self, bytes: &'a [u8]) -> &'a [u8] {
        let start = IMAGE_HEADER_SIZE;
        &bytes[start..start + self.payload_size as usize]
    }
}

/// Build an image out of a payload and the offset of its entry point.
///
/// The function is the counterpart of [`ImageHeader::parse`]: it is used by
/// the tests and by the code that builds the initial filesystem.
pub fn build(payload: &[u8], entry_offset: u32) -> Vec<u8> {
    let mut image = Vec::with_capacity(IMAGE_HEADER_SIZE + payload.len());
    image.extend_from_slice(&IMAGE_MAGIC);
    image.extend_from_slice(&IMAGE_VERSION.to_le_bytes());
    image.extend_from_slice(&entry_offset.to_le_bytes());
    image.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    image.extend_from_slice(payload);
    image
}

/// Read a little-endian number of four bytes.
fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}
