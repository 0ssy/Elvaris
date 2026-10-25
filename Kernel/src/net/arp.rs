//! ARP.

use alloc::collections::BTreeMap;
use crate::net::MacAddress;

pub const ARP_HTYPE_ETHERNET: u16 = 1;
pub const ARP_PTYPE_IP: u16 = 0x0800;
pub const ARP_HLEN_ETH: u8 = 6;
pub const ARP_PLEN_IP: u8 = 4;
pub const ARP_OP_REQUEST: u16 = 1;
pub const ARP_OP_REPLY: u16 = 2;

#[derive(Debug, Clone)]
pub struct ArpEntry {
    pub ip: [u8; 4],
    pub mac: MacAddress,
}

#[derive(Debug)]
pub struct ArpPacket {
    pub htype: u16,
    pub ptype: u16,
    pub hlen: u8,
    pub plen: u8,
    pub oper: u16,
    pub sha: MacAddress,
    pub spa: [u8; 4],
    pub tha: MacAddress,
    pub tpa: [u8; 4],
}

pub fn parse_arp(data: &[u8]) -> Option<ArpPacket> {
    if data.len() < 28 {
        return None;
    }
    let htype = ((data[0] as u16) << 8) | (data[1] as u16);
    let ptype = ((data[2] as u16) << 8) | (data[3] as u16);
    let hlen = data[4];
    let plen = data[5];
    if hlen != 6 || plen != 4 {
        return None;
    }
    let oper = ((data[6] as u16) << 8) | (data[7] as u16);
    let mut sha = [0u8; 6];
    let mut tha = [0u8; 6];
    sha.copy_from_slice(&data[8..14]);
    let mut spa = [0u8; 4];
    spa.copy_from_slice(&data[14..18]);
    tha.copy_from_slice(&data[18..24]);
    let mut tpa = [0u8; 4];
    tpa.copy_from_slice(&data[24..28]);
    Some(ArpPacket {
        htype,
        ptype,
        hlen,
        plen,
        oper,
        sha: MacAddress(sha),
        spa,
        tha: MacAddress(tha),
        tpa,
    })
}

pub fn build_arp_reply(sha: MacAddress, spa: [u8; 4], tha: MacAddress, tpa: [u8; 4]) -> alloc::vec::Vec<u8> {
    let mut pkt = alloc::vec![0u8; 28];
    pkt[0] = (ARP_HTYPE_ETHERNET >> 8) as u8;
    pkt[1] = (ARP_HTYPE_ETHERNET & 0xFF) as u8;
    pkt[2] = 0x08;
    pkt[3] = 0x00;
    pkt[4] = ARP_HLEN_ETH;
    pkt[5] = ARP_PLEN_IP;
    pkt[6] = (ARP_OP_REPLY >> 8) as u8;
    pkt[7] = (ARP_OP_REPLY & 0xFF) as u8;
    pkt[8..14].copy_from_slice(&sha.0);
    pkt[14..18].copy_from_slice(&spa);
    pkt[18..24].copy_from_slice(&tha.0);
    pkt[24..28].copy_from_slice(&tpa);
    pkt
}

pub struct ArpCache {
    entries: BTreeMap<u32, ArpEntry>,
    max_entries: usize,
}

impl ArpCache {
    pub fn new(max_entries: usize) -> Self {
        ArpCache {
            entries: BTreeMap::new(),
            max_entries,
        }
    }

    pub fn insert(&mut self, ip: [u8; 4], mac: MacAddress) {
        if self.entries.len() >= self.max_entries && !self.entries.contains_key(&ip_to_u32(ip)) {
            if let Some(key) = self.entries.keys().next().cloned() {
                self.entries.remove(&key);
            }
        }
        self.entries.insert(ip_to_u32(ip), ArpEntry { ip, mac });
    }

    pub fn lookup(&self, ip: [u8; 4]) -> Option<MacAddress> {
        self.entries.get(&ip_to_u32(ip)).map(|e| e.mac)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

fn ip_to_u32(ip: [u8; 4]) -> u32 {
    ((ip[0] as u32) << 24) | ((ip[1] as u32) << 16) | ((ip[2] as u32) << 8) | (ip[3] as u32)
}

