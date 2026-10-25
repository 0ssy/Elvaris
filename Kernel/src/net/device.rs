//! Network device abstraction.

use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    pub fn broadcast() -> Self {
        MacAddress([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
    }

    pub fn zero() -> Self {
        MacAddress([0, 0, 0, 0, 0, 0])
    }

    pub fn to_string(&self) -> [char; 17] {
        ['0'; 17]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetError {
    NotInitialized,
    NoDevice,
    QueueFull,
    QueueEmpty,
    InvalidFrame,
    LinkDown,
    TransmitFailed,
    ReceiveFailed,
    Unsupported,
    PermissionDenied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Down,
    Up,
}

pub trait NetworkDevice {
    fn init(&mut self) -> Result<(), NetError>;
    fn shutdown(&mut self) -> Result<(), NetError>;
    fn mac_address(&self) -> MacAddress;
    fn transmit(&mut self, frame: &[u8]) -> Result<(), NetError>;
    fn receive(&mut self) -> Option<Vec<u8>>;
    fn link_state(&self) -> LinkState;
    fn mtu(&self) -> usize;
    fn name(&self) -> &'static str;
}

/// Fake network device for testing
#[derive(Debug)]
pub struct FakeNetDevice {
    mac: MacAddress,
    initialized: bool,
    rx_queue: Vec<Vec<u8>>,
    tx_queue: Vec<Vec<u8>>,
}

impl FakeNetDevice {
    pub fn new() -> Self {
        FakeNetDevice {
            mac: MacAddress([0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]),
            initialized: false,
            rx_queue: Vec::new(),
            tx_queue: Vec::new(),
        }
    }

    pub fn inject_frame(&mut self, frame: Vec<u8>) {
        self.rx_queue.push(frame);
    }

    pub fn tx_captured(&self) -> &[Vec<u8>] {
        &self.tx_queue
    }
}

impl Default for FakeNetDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkDevice for FakeNetDevice {
    fn init(&mut self) -> Result<(), NetError> {
        self.initialized = true;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), NetError> {
        self.initialized = false;
        self.rx_queue.clear();
        self.tx_queue.clear();
        Ok(())
    }

    fn mac_address(&self) -> MacAddress {
        self.mac
    }

    fn transmit(&mut self, frame: &[u8]) -> Result<(), NetError> {
        if !self.initialized {
            return Err(NetError::NotInitialized);
        }
        if frame.len() > 1518 || frame.len() < 14 {
            return Err(NetError::InvalidFrame);
        }
        self.tx_queue.push(frame.to_vec());
        Ok(())
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        if !self.initialized || self.rx_queue.is_empty() {
            return None;
        }
        Some(self.rx_queue.remove(0))
    }

    fn link_state(&self) -> LinkState {
        if self.initialized { LinkState::Up } else { LinkState::Down }
    }

    fn mtu(&self) -> usize {
        1500
    }

    fn name(&self) -> &'static str {
        "fake-net"
    }
}

