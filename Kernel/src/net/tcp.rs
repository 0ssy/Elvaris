//! TCP.

use alloc::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    LastAck,
    Closing,
    TimeWait,
}

#[derive(Debug)]
pub struct TcpConnection {
    pub state: TcpState,
    pub local_port: u16,
    pub remote_ip: [u8; 4],
    pub remote_port: u16,
    pub seq: u32,
    pub ack: u32,
    pub backlog: usize,
}

pub struct TcpManager {
    conns: BTreeMap<(u16, [u8; 4], u16), TcpConnection>,
    listen_backlog: usize,
}

impl TcpManager {
    pub fn new() -> Self {
        TcpManager {
            conns: BTreeMap::new(),
            listen_backlog: 128,
        }
    }

    pub fn listen(&mut self, port: u16) {
        // Mark as listening
    }

    pub fn accept(&mut self) -> Option<TcpConnection> {
        None
    }
}

impl Default for TcpManager {
    fn default() -> Self {
        Self::new()
    }
}

