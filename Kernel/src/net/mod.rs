//! Networking stack for Elvaris (Milestone 15).

pub mod buffer;
pub mod device;
pub mod virtio_net;
pub mod ethernet;
pub mod arp;
pub mod ipv4;
pub mod icmp;
pub mod udp;
pub mod tcp;
pub mod socket;
pub mod service;

pub use buffer::{PacketBuffer, PacketError};
pub use device::{NetworkDevice, MacAddress, NetError};
