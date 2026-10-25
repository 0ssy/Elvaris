//! Network tests for Milestone 15.

#[cfg(test)]
mod tests {
    use crate::net::buffer::{PacketBuffer, PacketError, MAX_PACKET_SIZE};
    use crate::net::ethernet::parse_frame;
    use crate::net::arp::ArpCache;
    use crate::net::MacAddress;

    #[test]
    fn packet_buffer_basic() {
        let mut buf = PacketBuffer::new();
        assert_eq!(buf.len(), 0);
        assert!(buf.push(&[1, 2, 3]).is_ok());
        assert_eq!(buf.len(), 3);
    }

    #[test]
    fn packet_buffer_too_large() {
        let mut buf = PacketBuffer::with_capacity(10);
        let data = [0u8; 20];
        assert_eq!(buf.push(&data), Err(PacketError::TooLarge));
    }

    #[test]
    fn ethernet_parse() {
        let mut frame = [0u8; 14];
        frame[0..6].copy_from_slice(&[0xFF,0xFF,0xFF,0xFF,0xFF,0xFF]);
        frame[6..12].copy_from_slice(&[0x00,0x11,0x22,0x33,0x44,0x55]);
        frame[12] = 0x08; frame[13] = 0x06;
        let parsed = parse_frame(&frame).unwrap();
        assert_eq!(parsed.ethertype, 0x0806);
    }

    #[test]
    fn arp_cache() {
        let mut cache = ArpCache::new(10);
        cache.insert([192, 168, 1, 1], MacAddress([1,2,3,4,5,6]));
        assert!(cache.lookup([192, 168, 1, 1]).is_some());
    }
}
