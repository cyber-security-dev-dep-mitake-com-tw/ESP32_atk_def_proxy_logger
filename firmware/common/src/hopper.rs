//! Channel hopper — cycles a configured set of channels with a dwell time.
//! Pure logic; the node calls `next` on a timer and applies the channel to the radio.

use heapless::Vec;

/// Maximum channels in a hop set.
pub const MAX_HOP: usize = 14;

/// Cyclic channel hopper.
pub struct Hopper {
    channels: Vec<u8, MAX_HOP>,
    idx: usize,
    dwell_ms: u32,
    last_ms: u64,
}

impl Hopper {
    /// Create a hopper. Falls back to channel 1 if `channels` is empty.
    pub fn new(channels: &[u8], dwell_ms: u32) -> Self {
        let mut v = Vec::new();
        for &c in channels.iter().take(MAX_HOP) {
            let _ = v.push(c);
        }
        if v.is_empty() {
            let _ = v.push(1);
        }
        Hopper {
            channels: v,
            idx: 0,
            dwell_ms,
            last_ms: 0,
        }
    }

    /// Current channel.
    pub fn current(&self) -> u8 {
        self.channels[self.idx]
    }

    /// Advance to the next channel if `dwell_ms` has elapsed since the last hop.
    /// Returns Some(new_channel) when a hop occurs, else None.
    pub fn tick(&mut self, now_ms: u64) -> Option<u8> {
        if now_ms.saturating_sub(self.last_ms) < self.dwell_ms as u64 {
            return None;
        }
        self.last_ms = now_ms;
        self.idx = (self.idx + 1) % self.channels.len();
        Some(self.current())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hops_after_dwell() {
        let mut h = Hopper::new(&[1, 6, 11], 250);
        assert_eq!(h.current(), 1);
        assert_eq!(h.tick(100), None); // not enough time
        assert_eq!(h.tick(300), Some(6));
        assert_eq!(h.tick(600), Some(11));
        assert_eq!(h.tick(900), Some(1)); // wraps around
    }

    #[test]
    fn empty_channels_defaults_to_one() {
        let h = Hopper::new(&[], 100);
        assert_eq!(h.current(), 1);
    }
}
