//! Minimal 802.11 frame parsing — enough to classify management frames and pull
//! addresses for the deauth detector and PCAP annotation.

/// 802.11 frame type (bits 2..3 of the frame control field).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Management,
    Control,
    Data,
    Reserved,
}

/// Management-frame subtypes we care about (bits 4..7 of frame control).
pub const SUBTYPE_DEAUTH: u8 = 0x0C;
pub const SUBTYPE_DISASSOC: u8 = 0x0A;
pub const SUBTYPE_BEACON: u8 = 0x08;
pub const SUBTYPE_PROBE_REQ: u8 = 0x04;
pub const SUBTYPE_PROBE_RESP: u8 = 0x05;

/// A parsed 802.11 MAC header (the fields we use).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub frame_type: FrameType,
    pub subtype: u8,
    pub addr1: [u8; 6], // receiver / destination
    pub addr2: [u8; 6], // transmitter / source
    pub addr3: [u8; 6], // BSSID (typical for mgmt frames)
}

impl Frame {
    /// Parse the fixed portion of an 802.11 header. Returns None if the buffer is
    /// too short to contain the three addresses (24 bytes).
    pub fn parse(buf: &[u8]) -> Option<Frame> {
        if buf.len() < 24 {
            return None;
        }
        let fc = buf[0];
        let frame_type = match (fc >> 2) & 0x03 {
            0 => FrameType::Management,
            1 => FrameType::Control,
            2 => FrameType::Data,
            _ => FrameType::Reserved,
        };
        let subtype = (fc >> 4) & 0x0F;
        let mut addr1 = [0u8; 6];
        let mut addr2 = [0u8; 6];
        let mut addr3 = [0u8; 6];
        addr1.copy_from_slice(&buf[4..10]);
        addr2.copy_from_slice(&buf[10..16]);
        addr3.copy_from_slice(&buf[16..22]);
        Some(Frame {
            frame_type,
            subtype,
            addr1,
            addr2,
            addr3,
        })
    }

    /// True for deauthentication or disassociation management frames — the ones
    /// Node2 counts to detect a deauth attack.
    pub fn is_deauth_like(&self) -> bool {
        self.frame_type == FrameType::Management
            && (self.subtype == SUBTYPE_DEAUTH || self.subtype == SUBTYPE_DISASSOC)
    }
}

/// Format a MAC address as lowercase colon-separated hex into a fixed buffer.
pub fn fmt_mac(mac: &[u8; 6]) -> heapless::String<17> {
    let mut s = heapless::String::<17>::new();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (i, b) in mac.iter().enumerate() {
        if i > 0 {
            let _ = s.push(':');
        }
        let _ = s.push(HEX[(b >> 4) as usize] as char);
        let _ = s.push(HEX[(b & 0x0F) as usize] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deauth_frame() -> [u8; 24] {
        let mut f = [0u8; 24];
        // frame control: type=Management(00), subtype=Deauth(1100) => 0b1100_00_00 = 0xC0
        f[0] = 0xC0;
        f[4..10].copy_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]); // addr1
        f[10..16].copy_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]); // addr2
        f[16..22].copy_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]); // addr3/bssid
        f
    }

    #[test]
    fn parses_deauth() {
        let f = Frame::parse(&deauth_frame()).unwrap();
        assert_eq!(f.frame_type, FrameType::Management);
        assert_eq!(f.subtype, SUBTYPE_DEAUTH);
        assert!(f.is_deauth_like());
        assert_eq!(f.addr3, [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    }

    #[test]
    fn rejects_short_buffer() {
        assert!(Frame::parse(&[0u8; 10]).is_none());
    }

    #[test]
    fn beacon_is_not_deauth_like() {
        let mut f = [0u8; 24];
        f[0] = 0x80; // mgmt + beacon subtype
        let frame = Frame::parse(&f).unwrap();
        assert_eq!(frame.subtype, SUBTYPE_BEACON);
        assert!(!frame.is_deauth_like());
    }

    #[test]
    fn formats_mac() {
        assert_eq!(
            fmt_mac(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]).as_str(),
            "aa:bb:cc:dd:ee:ff"
        );
    }
}
