//! UDP.

#[derive(Debug)]
pub struct UdpPacket {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
    pub data: alloc::vec::Vec<u8>,
}

pub fn parse_udp(data: &[u8]) -> Option<UdpPacket> {
    if data.len() < 8 {
        return None;
    }
    let src_port = ((data[0] as u16) << 8) | (data[1] as u16);
    let dst_port = ((data[2] as u16) << 8) | (data[3] as u16);
    let length = ((data[4] as u16) << 8) | (data[5] as u16);
    if length < 8 || (length as usize) > data.len() {
        return None;
    }
    let checksum = ((data[6] as u16) << 8) | (data[7] as u16);
    let data_vec = data[8..(length as usize)].to_vec();
    Some(UdpPacket { src_port, dst_port, length, checksum, data: data_vec })
}

pub fn build_udp(src_port: u16, dst_port: u16, data: &[u8]) -> alloc::vec::Vec<u8> {
    let len = 8 + data.len() as u16;
    let mut pkt = alloc::vec![0u8; len as usize];
    pkt[0] = (src_port >> 8) as u8;
    pkt[1] = (src_port & 0xFF) as u8;
    pkt[2] = (dst_port >> 8) as u8;
    pkt[3] = (dst_port & 0xFF) as u8;
    pkt[4] = (len >> 8) as u8;
    pkt[5] = (len & 0xFF) as u8;
    pkt[6] = 0;
    pkt[7] = 0;
    pkt[8..].copy_from_slice(data);
    pkt
}

