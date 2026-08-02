# Firmware

Rust firmware for the three ESP32 nodes, built on **ESP-IDF** via `esp-idf-svc` /
`esp-idf-hal`, which exposes the promiscuous-mode WiFi API
(`esp_wifi_set_promiscuous_rx_cb`) and raw 802.11 TX (`esp_wifi_80211_tx`).

## Layout

- `common/` — hardware-independent logic, **host-tested in CI**:
  - `dot11` — 802.11 frame parsing / classification
  - `detector` — sliding-window deauth detector (Node2 core)
  - `hopper` — channel hopping
  - `radiotap` — radiotap header builder for PCAP
- `node1-monitor/` — promiscuous packet monitor, NDJSON over UART
- `node2-detector/` — deauth detector, NDJSON over WebSocket
- `node3-attacker/` — lab attack tester (own-network only; see ../SAFETY.md)

## Host testing (no hardware)

```bash
cargo test -p common      # runs the pure logic tests
cargo fmt -p common -- --check
```

## Building for the ESP32

Requires the Espressif Rust toolchain:

```bash
cargo install espup && espup install
cargo install ldproxy espflash
. ~/export-esp.sh                 # sets up the esp env

cd node1-monitor
cargo build --release             # uses rust-toolchain.toml (channel = "esp")
espflash flash --monitor target/xtensa-esp32-espidf/release/node1-monitor
```

Each node still needs a `.cargo/config.toml` selecting the xtensa target and the
ESP-IDF version. Generate one with the official template if you are starting fresh:

```bash
cargo generate esp-rs/esp-idf-template cargo
```

The template's `.cargo/config.toml` and `sdkconfig.defaults` are intentionally
omitted from this repo so the workspace stays toolchain-agnostic for host CI.

## Wiring to the PC

- **Node1**: USB serial to the PC; the Go agent reads it with `--node1-serial`.
- **Node2 / Node3**: join your LAN over WiFi STA and open a WebSocket to
  `ws://<agent-host>:8080/ws/node/node2` (resp. `node3`).
