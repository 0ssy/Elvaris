//! ICMP.

#[derive(Debug)]
pub struct IcmpPacket {
    pub icmp_type: u8,
    pub code: u8,
    pub checksum: u16,
    pub data: alloc::vec::Vec<u8>,
}

pub fn parse_icmp(data: &[u8]) -> Option<IcmpPacket> {
    if data.len() < 8 {
        return None;
    }
    let checksum = ((data[2] as u16) << 8) | (data[3] as u16);
    Some(IcmpPacket {
        icmp_type: data[0],
        code: data[1],
        checksum,
        data: data[8..].to_vec(),
    })
}

pub fn build_echo_reply(identifier: u16, seq: u16, payload: &[u8]) -> alloc::vec::Vec<u8> {
    let mut pkt = alloc::vec![0u8; 8 + payload.len()];
    pkt[0] = ICMP_ECHO_REPLY;
    pkt[1] = 0;
    pkt[2] = 0;
    pkt[3] = 0;
    pkt[4] = (identifier >> 8) as u8;
    pkt[5] = (identifier & 0xFF) as u8;
    pkt[6] = (seq >> 8) as u8;
    pkt[7] = (seq & 0xFF) as u8;
    pkt[8..].copy_from_slice(payload);
    let csum = crate::net::ipv4::checksum(&pkt);
    pkt[2] = (csum >> 8) as u8;
    pkt[3] = (csum & 0xFF) as u8;
    pkt
}

pub const ICMP_ECHO_REQUEST: u8 = 8;
pub const ICMP_ECHO_REPLY: u8 = 0;

