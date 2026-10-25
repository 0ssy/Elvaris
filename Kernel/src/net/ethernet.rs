//! Ethernet frame handling.

use crate::net::MacAddress;

pub const ETHERTYPE_IP: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;

#[derive(Debug)]
pub struct EthernetFrame {
    pub dst: MacAddress,
    pub src: MacAddress,
    pub ethertype: u16,
    pub payload: alloc::vec::Vec<u8>,
}

pub fn parse_frame(data: &[u8]) -> Option<EthernetFrame> {
    if data.len() < 14 {
        return None;
    }
    let mut dst = [0u8; 6];
    let mut src = [0u8; 6];
    dst.copy_from_slice(&data[0..6]);
    src.copy_from_slice(&data[6..12]);
    let ethertype = ((data[12] as u16) << 8) | (data[13] as u16);
    if data.len() > 1518 {
        return None;
    }
    let payload = data[14..].to_vec();
    Some(EthernetFrame {
        dst: MacAddress(dst),
        src: MacAddress(src),
        ethertype,
        payload,
    })
}

pub fn build_frame(dst: MacAddress, src: MacAddress, ethertype: u16, payload: &[u8]) -> alloc::vec::Vec<u8> {
    let mut frame = alloc::vec![0u8; 14 + payload.len()];
    frame[0..6].copy_from_slice(&dst.0);
    frame[6..12].copy_from_slice(&src.0);
    frame[12] = (ethertype >> 8) as u8;
    frame[13] = (ethertype & 0xFF) as u8;
    frame[14..].copy_from_slice(payload);
    frame
}

