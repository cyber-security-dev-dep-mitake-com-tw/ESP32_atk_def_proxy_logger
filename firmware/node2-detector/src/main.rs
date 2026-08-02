//! Node2 — Deauth detector + alert.
//!
//! Filters promiscuous RX to deauth/disassoc management frames and runs the
//! sliding-window detector from `common`. When an attack is detected it emits a
//! `deauth_alert` NDJSON event over its WebSocket to the PC agent.
//!
//! ESP-IDF scaffold; the detection logic lives in `common::detector` and is
//! host-tested. Build with the esp toolchain (see firmware/README.md).

use common::detector::{Config, Detector};
use common::dot11::Frame;

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("node2-detector starting: deauth detector");

    let mut detector = Detector::new(Config { threshold: 5, window_ms: 1000 });

    // Real firmware:
    //   1. connect WiFi STA to LAN, open WebSocket to ws://<agent>/ws/node/node2
    //   2. enable promiscuous mode with a filter mask for management frames
    //   3. in rx_cb: parse the frame; if is_deauth_like, feed detector.observe(bssid, now_ms)
    //   4. on Some(alert), send a deauth_alert event over the WebSocket
    //   5. handle inbound commands (start_deauth_detect adjusts threshold/window).

    // Illustrative use of the shared logic so the scaffold references real APIs.
    let demo_frame = [0u8; 24];
    if let Some(f) = Frame::parse(&demo_frame) {
        if f.is_deauth_like() {
            let _ = detector.observe(f.addr3, 0);
        }
    }

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
