//! Node3 — Lab attack tester (OWN NETWORK ONLY).
//!
//! Sends raw 802.11 frames (deauth, beacon spam) for security testing against
//! networks you own and control. A firmware-side safety gate refuses any TX to a
//! BSSID not on the compiled-in `OWN_NETWORKS` allowlist, mirroring the backend.
//! Attacks are DISABLED until explicitly armed over the control channel.
//!
//! Use only on your own network in a lab environment. Deauthing networks you do
//! not own is illegal in most jurisdictions.

/// Compiled-in allowlist of BSSIDs you own. TX to anything else is refused.
/// Replace with your own AP MACs before building.
const OWN_NETWORKS: &[[u8; 6]] = &[
    // e.g. [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF],
];

/// Returns true only if `bssid` is a network we are permitted to test against.
fn is_own_network(bssid: &[u8; 6]) -> bool {
    OWN_NETWORKS.iter().any(|b| b == bssid)
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::warn!("node3-attacker starting: LAB attacks, own-network only, disarmed");

    // Real firmware:
    //   1. connect WiFi STA, open WebSocket to ws://<agent>/ws/node/node3
    //   2. stay disarmed until an `attack` command with confirm_own_net arrives
    //   3. gate every TX through is_own_network(bssid) — refuse + log otherwise
    //   4. esp_wifi_80211_tx(...) to send the crafted frame
    //   5. echo every TX as a `log` event for the audit trail.

    let target = [0u8; 6];
    if is_own_network(&target) {
        log::info!("would transmit test frame to own network");
    } else {
        log::warn!("refusing TX: target not on OWN_NETWORKS allowlist");
    }

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_unknown_bssid() {
        assert!(!is_own_network(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]));
    }
}
