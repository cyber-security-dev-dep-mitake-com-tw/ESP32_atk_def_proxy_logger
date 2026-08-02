//! Sliding-window deauth attack detector (Node2 core logic).
//!
//! Counts deauth/disassoc frames per BSSID within a time window. When the count
//! crosses a threshold, an alert should be emitted. Time is passed in explicitly
//! (millis since boot) so the logic is deterministic and host-testable.

use heapless::FnvIndexMap;

/// Maximum distinct BSSIDs tracked at once (power of two for heapless map).
pub const MAX_BSSIDS: usize = 32;

/// One tracked BSSID's rolling state.
#[derive(Clone, Copy)]
struct Window {
    window_start_ms: u64,
    count: u32,
    alerted: bool,
}

/// Detector configuration.
#[derive(Clone, Copy)]
pub struct Config {
    pub threshold: u32,
    pub window_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            threshold: 5,
            window_ms: 1000,
        }
    }
}

/// A raised alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alert {
    pub bssid: [u8; 6],
    pub count: u32,
}

/// Deauth detector holding per-BSSID sliding windows.
pub struct Detector {
    cfg: Config,
    windows: FnvIndexMap<[u8; 6], Window, MAX_BSSIDS>,
}

impl Detector {
    pub fn new(cfg: Config) -> Self {
        Detector {
            cfg,
            windows: FnvIndexMap::new(),
        }
    }

    /// Record a deauth-like frame for `bssid` at `now_ms`. Returns Some(Alert)
    /// exactly once per window when the threshold is first crossed.
    pub fn observe(&mut self, bssid: [u8; 6], now_ms: u64) -> Option<Alert> {
        let w = match self.windows.get(&bssid).copied() {
            Some(mut w) => {
                if now_ms.saturating_sub(w.window_start_ms) > self.cfg.window_ms {
                    // Window expired: start a fresh one.
                    w.window_start_ms = now_ms;
                    w.count = 0;
                    w.alerted = false;
                }
                w.count += 1;
                w
            }
            None => Window {
                window_start_ms: now_ms,
                count: 1,
                alerted: false,
            },
        };

        let mut w = w;
        let mut alert = None;
        if !w.alerted && w.count >= self.cfg.threshold {
            w.alerted = true;
            alert = Some(Alert {
                bssid,
                count: w.count,
            });
        }
        // Insert may fail only if the map is full and this is a new key; drop silently.
        let _ = self.windows.insert(bssid, w);
        alert
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BSSID: [u8; 6] = [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];

    #[test]
    fn alerts_once_when_threshold_crossed() {
        let mut d = Detector::new(Config {
            threshold: 3,
            window_ms: 1000,
        });
        assert_eq!(d.observe(BSSID, 0), None);
        assert_eq!(d.observe(BSSID, 100), None);
        let a = d.observe(BSSID, 200).expect("should alert on 3rd");
        assert_eq!(a.count, 3);
        // No repeat alert within the same window.
        assert_eq!(d.observe(BSSID, 300), None);
    }

    #[test]
    fn window_resets_after_expiry() {
        let mut d = Detector::new(Config {
            threshold: 2,
            window_ms: 1000,
        });
        d.observe(BSSID, 0);
        d.observe(BSSID, 100).expect("alert");
        // Past the window: counting restarts, so a single frame does not alert.
        assert_eq!(d.observe(BSSID, 2000), None);
        // Second frame in the new window alerts again.
        assert!(d.observe(BSSID, 2100).is_some());
    }

    #[test]
    fn distinct_bssids_tracked_independently() {
        let mut d = Detector::new(Config {
            threshold: 2,
            window_ms: 1000,
        });
        let other = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        assert_eq!(d.observe(BSSID, 0), None);
        assert_eq!(d.observe(other, 0), None);
        assert!(d.observe(BSSID, 10).is_some());
        assert!(d.observe(other, 10).is_some());
    }
}
