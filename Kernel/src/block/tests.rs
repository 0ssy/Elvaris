//! Unit tests for the block-device interface.
//!
//! The tests run on the host and use the memory-backed device. They cover
//! the rules the interface promises: the buffer length, the block range, the
//! isolation of the blocks, and the behaviour of a device that is not ready.

use super::{BLOCK_SIZE, BlockDevice, BlockError, MemoryBlockDevice};

/// A device with eight blocks.
fn device() -> MemoryBlockDevice {
    MemoryBlockDevice::new(8)
}

/// A buffer of exactly one block.
fn block(value: u8) -> [u8; BLOCK_SIZE] {
    [value; BLOCK_SIZE]
}

#[test]
fn a_new_device_is_empty_and_ready() {
    let device = device();
    assert_eq!(device.block_size(), BLOCK_SIZE);
    assert_eq!(device.block_count(), 8);
    assert_eq!(device.capacity_bytes(), 8 * BLOCK_SIZE);
    assert!(device.is_ready());
    assert_eq!(device.device_id(), "memory");

    let mut buffer = [0xFFu8; BLOCK_SIZE];
    device
        .read_block(0, &mut buffer)
        .expect("the first block reads");
    assert!(buffer.iter().all(|byte| *byte == 0));
}

#[test]
fn a_block_can_be_written_and_read_back() {
    let mut device = device();
    device
        .write_block(3, &block(0xAB))
        .expect("the write succeeds");

    let mut buffer = [0u8; BLOCK_SIZE];
    device
        .read_block(3, &mut buffer)
        .expect("the read succeeds");
    assert!(buffer.iter().all(|byte| *byte == 0xAB));
}

#[test]
fn reading_after_writing_returns_the_same_bytes() {
    let mut device = device();
    let payload: Vec<u8> = (0..BLOCK_SIZE).map(|index| (index % 251) as u8).collect();
    device.write_block(0, &payload).expect("the write succeeds");

    let mut buffer = vec![0u8; BLOCK_SIZE];
    device
        .read_block(0, &mut buffer)
        .expect("the read succeeds");
    assert_eq!(buffer, payload);
}

#[test]
fn a_block_out_of_range_is_refused() {
    let mut device = device();
    let mut buffer = [0u8; BLOCK_SIZE];

    assert_eq!(
        device.read_block(8, &mut buffer),
        Err(BlockError::OutOfRange)
    );
    assert_eq!(
        device.read_block(usize::MAX, &mut buffer),
        Err(BlockError::OutOfRange)
    );
    assert_eq!(
        device.write_block(8, &block(1)),
        Err(BlockError::OutOfRange)
    );
    assert_eq!(
        device.write_block(usize::MAX, &block(1)),
        Err(BlockError::OutOfRange)
    );
}

#[test]
fn a_buffer_of_the_wrong_length_is_refused() {
    let mut device = device();
    let mut short = [0u8; BLOCK_SIZE - 1];
    let mut long = [0u8; BLOCK_SIZE + 1];
    let mut empty: [u8; 0] = [];

    assert_eq!(
        device.read_block(0, &mut short),
        Err(BlockError::BadBufferSize)
    );
    assert_eq!(
        device.read_block(0, &mut long),
        Err(BlockError::BadBufferSize)
    );
    assert_eq!(
        device.read_block(0, &mut empty),
        Err(BlockError::BadBufferSize)
    );
    assert_eq!(
        device.write_block(0, &short),
        Err(BlockError::BadBufferSize)
    );
    assert_eq!(device.write_block(0, &long), Err(BlockError::BadBufferSize));
}

#[test]
fn a_refused_write_does_not_change_the_block() {
    let mut device = device();
    device
        .write_block(1, &block(0x11))
        .expect("the write succeeds");

    let mut too_long = [0x22u8; BLOCK_SIZE + 1];
    assert_eq!(
        device.write_block(1, &too_long),
        Err(BlockError::BadBufferSize)
    );

    let mut buffer = [0u8; BLOCK_SIZE];
    device
        .read_block(1, &mut buffer)
        .expect("the read succeeds");
    assert!(buffer.iter().all(|byte| *byte == 0x11));
}

#[test]
fn the_blocks_are_isolated_from_each_other() {
    let mut device = device();
    device
        .write_block(0, &block(0x01))
        .expect("the write succeeds");
    device
        .write_block(1, &block(0x02))
        .expect("the write succeeds");
    device
        .write_block(7, &block(0x07))
        .expect("the write succeeds");

    let mut buffer = [0u8; BLOCK_SIZE];
    device
        .read_block(0, &mut buffer)
        .expect("the read succeeds");
    assert!(buffer.iter().all(|byte| *byte == 0x01));

    device
        .read_block(1, &mut buffer)
        .expect("the read succeeds");
    assert!(buffer.iter().all(|byte| *byte == 0x02));

    device
        .read_block(7, &mut buffer)
        .expect("the read succeeds");
    assert!(buffer.iter().all(|byte| *byte == 0x07));

    // The blocks that were never written are still empty.
    device
        .read_block(3, &mut buffer)
        .expect("the read succeeds");
    assert!(buffer.iter().all(|byte| *byte == 0));
}

#[test]
fn a_repeated_write_replaces_the_whole_block() {
    let mut device = device();
    device
        .write_block(2, &block(0xAA))
        .expect("the write succeeds");
    device
        .write_block(2, &block(0xBB))
        .expect("the second write succeeds");

    let mut buffer = [0u8; BLOCK_SIZE];
    device
        .read_block(2, &mut buffer)
        .expect("the read succeeds");
    assert!(buffer.iter().all(|byte| *byte == 0xBB));
}

#[test]
fn an_empty_device_refuses_every_access() {
    let mut device = MemoryBlockDevice::new(0);
    assert_eq!(device.block_count(), 0);
    assert_eq!(device.capacity_bytes(), 0);

    let mut buffer = [0u8; BLOCK_SIZE];
    assert_eq!(
        device.read_block(0, &mut buffer),
        Err(BlockError::OutOfRange)
    );
    assert_eq!(
        device.write_block(0, &block(1)),
        Err(BlockError::OutOfRange)
    );
}

#[test]
fn a_device_that_is_not_ready_refuses_every_transfer() {
    let mut device = device();
    device.set_ready(false);
    assert!(!device.is_ready());

    let mut buffer = [0u8; BLOCK_SIZE];
    assert_eq!(device.read_block(0, &mut buffer), Err(BlockError::NotReady));
    assert_eq!(device.write_block(0, &block(1)), Err(BlockError::NotReady));
}

#[test]
fn several_blocks_are_transferred_at_once() {
    let mut device = device();
    let mut payload = Vec::new();
    for index in 0..3u8 {
        payload.extend_from_slice(&block(index));
    }
    device
        .write_blocks(2, &payload)
        .expect("the write succeeds");

    let mut buffer = vec![0u8; 3 * BLOCK_SIZE];
    device
        .read_blocks(2, &mut buffer)
        .expect("the read succeeds");
    assert_eq!(buffer, payload);
}

#[test]
fn a_multi_block_transfer_that_leaves_the_device_is_refused() {
    let mut device = device();
    let payload = vec![0u8; 3 * BLOCK_SIZE];

    assert_eq!(
        device.write_blocks(6, &payload),
        Err(BlockError::OutOfRange)
    );
    let mut buffer = vec![0u8; 3 * BLOCK_SIZE];
    assert_eq!(
        device.read_blocks(6, &mut buffer),
        Err(BlockError::OutOfRange)
    );
}

#[test]
fn a_multi_block_transfer_with_a_partial_block_is_refused() {
    let mut device = device();
    let payload = vec![0u8; BLOCK_SIZE + 1];

    assert_eq!(
        device.write_blocks(0, &payload),
        Err(BlockError::BadBufferSize)
    );
    let mut buffer = vec![0u8; BLOCK_SIZE + 1];
    assert_eq!(
        device.read_blocks(0, &mut buffer),
        Err(BlockError::BadBufferSize)
    );
}

#[test]
fn a_device_can_be_created_from_bytes() {
    let device = MemoryBlockDevice::from_bytes(&[1, 2, 3, 4]).expect("the device is created");
    // Four bytes still occupy a whole block.
    assert_eq!(device.block_count(), 1);

    let mut buffer = [0u8; BLOCK_SIZE];
    device
        .read_block(0, &mut buffer)
        .expect("the read succeeds");
    assert_eq!(&buffer[..4], &[1, 2, 3, 4]);
    assert!(buffer[4..].iter().all(|byte| *byte == 0));
}

#[test]
fn the_contents_of_a_device_can_be_read_back_as_bytes() {
    let mut device = MemoryBlockDevice::new(2);
    device
        .write_block(1, &block(0x5A))
        .expect("the write succeeds");

    let bytes = device.to_bytes();
    assert_eq!(bytes.len(), 2 * BLOCK_SIZE);
    assert!(bytes[..BLOCK_SIZE].iter().all(|byte| *byte == 0));
    assert!(bytes[BLOCK_SIZE..].iter().all(|byte| *byte == 0x5A));
}

#[test]
fn flushing_a_memory_device_succeeds() {
    let mut device = device();
    assert_eq!(device.flush(), Ok(()));
}
