//! Shared, hardware-independent logic for all three ESP32 nodes.
//!
//! Everything here is plain Rust with no ESP-IDF dependency so it can be unit
//! tested on the host. The node binaries pull this in and wire it to the radio.

pub mod detector;
pub mod dot11;
pub mod hopper;
pub mod radiotap;

/// WiFi 2.4 GHz channels legal in most regions (1..=13). Node firmwares pick a
/// subset (commonly 1, 6, 11) for hopping.
pub const CHANNELS_2G: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
