//! Network service.

use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub ip: [u8; 4],
    pub netmask: [u8; 4],
    pub gateway: [u8; 4],
    pub dns: [u8; 4],
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            ip: [10, 0, 2, 15],
            netmask: [255, 255, 255, 0],
            gateway: [10, 0, 2, 2],
            dns: [8, 8, 8, 8],
        }
    }
}

pub fn init_network() {
    unsafe {
        crate::serial::write_str("Elvaris kernel: network: initializing\r\n");
    }
}
