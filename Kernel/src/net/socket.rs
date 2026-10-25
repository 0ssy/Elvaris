//! Sockets.

use alloc::vec::Vec;
use alloc::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Stream,
    Dgram,
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Unbound,
    Bound,
    Listening,
    Connected,
    Closed,
}

#[derive(Debug)]
pub struct Socket {
    pub socket_type: SocketType,
    pub state: SocketState,
    pub local_ip: [u8; 4],
    pub local_port: u16,
    pub remote_ip: [u8; 4],
    pub remote_port: u16,
    pub owner_pid: u64,
    pub recv_queue: Vec<Vec<u8>>,
}

impl Socket {
    pub fn new(socket_type: SocketType, owner_pid: u64) -> Self {
        Socket {
            socket_type,
            state: SocketState::Unbound,
            local_ip: [0, 0, 0, 0],
            local_port: 0,
            remote_ip: [0, 0, 0, 0],
            remote_port: 0,
            owner_pid,
            recv_queue: Vec::new(),
        }
    }

    pub fn bind(&mut self, ip: [u8; 4], port: u16) -> bool {
        if self.state != SocketState::Unbound {
            return false;
        }
        self.local_ip = ip;
        self.local_port = port;
        self.state = SocketState::Bound;
        true
    }

    pub fn close(&mut self) {
        self.state = SocketState::Closed;
        self.recv_queue.clear();
    }
}

pub struct SocketTable {
    sockets: BTreeMap<u64, Socket>,
    next_id: u64,
    max_per_process: usize,
    max_total: usize,
}

impl SocketTable {
    pub fn new() -> Self {
        SocketTable {
            sockets: BTreeMap::new(),
            next_id: 1,
            max_per_process: 64,
            max_total: 1024,
        }
    }

    pub fn create(&mut self, socket_type: SocketType, owner_pid: u64) -> Option<u64> {
        if self.sockets.len() >= self.max_total {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.sockets.insert(id, Socket::new(socket_type, owner_pid));
        Some(id)
    }
}

impl Default for SocketTable {
    fn default() -> Self {
        Self::new()
    }
}

