//! VirtIO-net driver (stub for Milestone 15).

use crate::net::{MacAddress, NetError, NetworkDevice};

pub struct VirtioNetDevice {
    mac: MacAddress,
    initialized: bool,
}

impl VirtioNetDevice {
    pub fn new() -> Self {
        VirtioNetDevice {
            mac: MacAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]),
            initialized: false,
        }
    }
}

impl Default for VirtioNetDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkDevice for VirtioNetDevice {
    fn init(&mut self) -> Result<(), NetError> {
        self.initialized = true;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), NetError> {
        self.initialized = false;
        Ok(())
    }

    fn mac_address(&self) -> MacAddress {
        self.mac
    }

    fn transmit(&mut self, _frame: &[u8]) -> Result<(), NetError> {
        if !self.initialized {
            return Err(NetError::NotInitialized);
        }
        Ok(())
    }

    fn receive(&mut self) -> Option<alloc::vec::Vec<u8>> {
        if !self.initialized {
            return None;
        }
        None
    }

    fn link_state(&self) -> crate::net::device::LinkState {
        if self.initialized { crate::net::device::LinkState::Up } else { crate::net::device::LinkState::Down }
    }

    fn mtu(&self) -> usize {
        1500
    }

    fn name(&self) -> &'static str {
        "virtio-net"
    }
}
