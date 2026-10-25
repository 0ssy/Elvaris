//! Network device abstraction.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    pub fn broadcast() -> Self {
        MacAddress([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
    }

    pub fn zero() -> Self {
        MacAddress([0, 0, 0, 0, 0, 0])
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
    fn receive(&mut self) -> Option<alloc::vec::Vec<u8>>;
    fn link_state(&self) -> LinkState;
    fn mtu(&self) -> usize;
    fn name(&self) -> &'static str;
}
