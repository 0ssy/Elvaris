use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetMode {
    Static,
    Dhcp,
    Offline,
}

#[derive(Debug, Clone)]
pub struct EmbeddedNetProfile {
    pub mode: NetMode,
    pub interface: String,
    pub allow_listen: bool,
    pub allowed_ports: Vec<u16>,
    pub require_network: bool,
}

impl EmbeddedNetProfile {
    pub fn new() -> Self {
        Self {
            mode: NetMode::Static,
            interface: "eth0".to_string(),
            allow_listen: false,
            allowed_ports: Vec::new(),
            require_network: false,
        }
    }

    pub fn offline() -> Self {
        Self {
            mode: NetMode::Offline,
            interface: "eth0".to_string(),
            allow_listen: false,
            allowed_ports: Vec::new(),
            require_network: false,
        }
    }

    pub fn can_listen_on(&self, port: u16) -> bool {
        self.allow_listen && self.allowed_ports.contains(&port)
    }

    pub fn can_connect(&self) -> bool {
        self.mode != NetMode::Offline || !self.require_network
    }
}

impl Default for EmbeddedNetProfile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_defaults() {
        let profile = EmbeddedNetProfile::new();
        assert!(!profile.allow_listen);
        assert!(profile.allowed_ports.is_empty());
        assert!(profile.can_connect());
    }

    #[test]
    fn offline_profile() {
        let profile = EmbeddedNetProfile::offline();
        assert_eq!(profile.mode, NetMode::Offline);
    }
}
