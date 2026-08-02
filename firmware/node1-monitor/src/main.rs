//! Node1 — Packet Monitor + PCAP source.
//!
//! Puts the ESP32 WiFi into promiscuous mode, registers an RX callback, and for
//! every captured frame emits a base64 NDJSON `packet` event (radiotap + 802.11)
//! over UART. The PC-side Go agent wraps these into a PCAP file.
//!
//! This file is an ESP-IDF scaffold: the hardware-independent pieces (radiotap
//! building, channel hopping, frame parsing) live in the `common` crate and are
//! unit-tested on the host. Build with the esp toolchain (see firmware/README.md).

use common::{hopper::Hopper, radiotap};

fn main() {
    // Required once at startup for esp-idf-svc apps.
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("node1-monitor starting: promiscuous packet monitor");

    // Hop 1/6/11 with a 250 ms dwell by default; the PC can reconfigure via NDJSON.
    let mut hopper = Hopper::new(&[1, 6, 11], 250);
    log::info!("initial channel {}", hopper.current());

    // In the real firmware:
    //   1. init NVS + WiFi driver, esp_wifi_set_promiscuous(true)
    //   2. esp_wifi_set_promiscuous_rx_cb(rx_cb)
    //   3. in rx_cb: parse header, build radiotap, base64-encode, print NDJSON line
    //   4. read UART for commands (set_channel / start_hop / get_stats)
    //   5. on a timer, call hopper.tick(now_ms) and apply the channel.
    let _ = radiotap::build(hopper.current(), -42);

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
