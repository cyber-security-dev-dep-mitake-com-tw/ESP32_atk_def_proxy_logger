//! Builds a minimal radiotap header to prepend to captured 802.11 frames so the
//! resulting PCAP (LINKTYPE_IEEE802_11_RADIOTAP) carries channel and RSSI in a
//! form Wireshark understands.

/// Radiotap "present" flags for the fields we include.
const PRESENT_FLAGS: u32 = 1 << 1; // TSFT off, FLAGS off... we use CHANNEL + ANTSIGNAL
const FIELD_CHANNEL: u32 = 1 << 3;
const FIELD_ANTSIGNAL: u32 = 1 << 5;

/// Build a radiotap header carrying channel frequency and antenna signal (dBm).
/// Layout: [version=0][pad=0][len u16 LE][present u32 LE][chan_freq u16 LE][chan_flags u16 LE][antsignal i8].
pub fn build(channel: u8, rssi_dbm: i8) -> heapless::Vec<u8, 16> {
    let mut v = heapless::Vec::<u8, 16>::new();
    let present = FIELD_CHANNEL | FIELD_ANTSIGNAL | (PRESENT_FLAGS & 0);
    let freq = channel_to_freq(channel);

    let _ = v.push(0); // version
    let _ = v.push(0); // pad
                       // length placeholder (fill after)
    let _ = v.push(0);
    let _ = v.push(0);
    push_u32_le(&mut v, present);
    push_u16_le(&mut v, freq);
    push_u16_le(&mut v, 0x00a0); // channel flags: 2GHz
    let _ = v.push(rssi_dbm as u8);

    let len = v.len() as u16;
    v[2] = (len & 0xFF) as u8;
    v[3] = (len >> 8) as u8;
    v
}

/// Convert a 2.4 GHz channel number to its center frequency in MHz.
pub fn channel_to_freq(ch: u8) -> u16 {
    match ch {
        14 => 2484,
        c => 2407 + (c as u16) * 5,
    }
}

fn push_u16_le(v: &mut heapless::Vec<u8, 16>, x: u16) {
    let _ = v.push((x & 0xFF) as u8);
    let _ = v.push((x >> 8) as u8);
}

fn push_u32_le(v: &mut heapless::Vec<u8, 16>, x: u32) {
    for i in 0..4 {
        let _ = v.push(((x >> (i * 8)) & 0xFF) as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_length_field_matches() {
        let h = build(6, -42);
        let len = u16::from_le_bytes([h[2], h[3]]) as usize;
        assert_eq!(len, h.len());
    }

    #[test]
    fn channel_frequencies() {
        assert_eq!(channel_to_freq(1), 2412);
        assert_eq!(channel_to_freq(6), 2437);
        assert_eq!(channel_to_freq(11), 2462);
        assert_eq!(channel_to_freq(14), 2484);
    }

    #[test]
    fn rssi_roundtrips_as_signed() {
        let h = build(1, -30);
        let sig = *h.last().unwrap() as i8;
        assert_eq!(sig, -30);
    }
}
