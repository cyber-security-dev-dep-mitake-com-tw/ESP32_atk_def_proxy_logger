# Firmware

Rust firmware for the three ESP32-S3 nodes, built on **ESP-IDF** via `esp-idf-svc` /
`esp-idf-hal`, which expose the promiscuous-mode WiFi API
(`esp_wifi_set_promiscuous_rx_cb`) and raw 802.11 TX (`esp_wifi_80211_tx`).

All three nodes are flashed and verified on real **ESP32-S3** hardware (native USB,
no UART bridge). Hardware seen in this project:

| Node  | Role                        | Transport        | Identify LED | Board MAC            |
|-------|-----------------------------|------------------|--------------|---------------------|
| Node1 | Packet monitor + PCAP       | USB serial       | —            | `14:c1:9f:cb:51:b4` |
| Node2 | Deauth detector + alert     | USB serial       | **blue** blink | `14:c1:9f:26:36:00` |
| Node3 | Lab attacker (own net only) | **WiFi WebSocket** | **green** blink | `e8:3d:c1:f1:4b:c0` |

## Layout

- `common/` — hardware-independent logic, **host-tested in CI**:
  - `dot11` — 802.11 frame parsing / classification
  - `detector` — sliding-window deauth detector (Node2 core)
  - `hopper` — channel hopping
  - `radiotap` — radiotap header builder for PCAP
- `node1-monitor/` — promiscuous packet monitor → radiotap+base64 NDJSON over USB serial
- `node2-detector/` — deauth detector → `deauth_alert` NDJSON over USB serial; blue WS2812 LED
- `node3-attacker/` — WiFi-STA + WebSocket client, deauth injection, green WS2812 LED
  (own-network only; see [../SAFETY.md](../SAFETY.md))

Each node ships its own `.cargo/config.toml` (Xtensa target, USB-JTAG console via the
workspace `sdkconfig.defaults`) and `rust-toolchain.toml` (`channel = "esp"`).

## Toolchain (one-time)

```bash
cargo install espup && espup install     # installs the esp Xtensa toolchain
cargo install ldproxy espflash
source ~/export-esp.sh                    # sets LIBCLANG_PATH + xtensa gcc on PATH
```

## Host testing (no hardware)

```bash
cargo test -p common          # pure logic tests
cargo fmt -p common -- --check
```

## Building & flashing

Workspace builds land in `firmware/target/xtensa-esp32s3-espidf/release/<node>`
(NOT the per-crate dir). The first build downloads ESP-IDF v5.2.3 (~1 GB) and takes
several minutes; later builds are fast.

Helper scripts handle build + flash per node:

```bash
./flash-node1.sh    # packet monitor
./flash-node2.sh    # deauth detector
./flash-node3.sh    # attacker (needs .wifi.env first — see below)
```

### ESP32-S3 native-USB flashing quirks

- **Download mode**: some S3 boards don't auto-reset into the ROM bootloader. If
  `espflash` prints `Error while connecting`, enter download mode by hand — **hold
  BOOT, tap RESET/EN, release BOOT** — then flash. In download mode the ROM appears
  as a *different* port (e.g. `/dev/cu.usbmodem1101`).
- **Use the stub, not `--no-stub`**: `--no-stub` returned "bootloader returned an
  error"; the working flash keeps the stub:
  ```bash
  espflash flash --before no-reset --after hard-reset --port /dev/cu.usbmodem1101 \
    target/xtensa-esp32s3-espidf/release/node1-monitor
  ```
- **Stuck in download mode**: no software reset over USB clears the download latch —
  **power-cycle** (unplug ~5 s, replug, BOOT not held) to boot the flashed app.
- **DTR = GPIO0 trap**: on the USB-Serial/JTAG, DTR is wired to BOOT and RTS to EN.
  The Go agent opens the port with **both held low** (`SetDTR(false)`/`SetRTS(false)`)
  so opening it neither resets the board nor forces download mode.

## Node1 / Node2 — USB serial

```bash
# Node1 packet monitor → PCAP in data/
cd ../backend && go run ./cmd/agent --node1-serial /dev/cu.usbmodem1101 --baud 115200

# Node2 deauth detector (blue LED heartbeat, deauth_alert on attack)
go run ./cmd/agent --node2-serial /dev/cu.usbmodem1101
```

> **Two boards, one port**: Node1 and Node2 both present the *same* USB-Serial/JTAG
> identity, so only one `/dev/cu.usbmodem1101` appears at a time. Run them one at a
> time, or re-plug so each re-enumerates before using `--node1-serial` +
> `--node2-serial` together.

## Node3 — WiFi WebSocket + deauth injection (OWN NETWORK ONLY)

Node3 joins your WiFi as a station, dials the backend over a WebSocket, and injects
deauth frames **only** against an allowlisted BSSID. It runs wirelessly after
flashing — no USB port needed — which also sidesteps the shared-port issue above.

### 1. Credentials (`.wifi.env`, gitignored)

```bash
cp node3-attacker/.wifi.env.example node3-attacker/.wifi.env
# edit it:
#   WIFI_SSID / WIFI_PASSWORD  — your lab network (2.4 GHz!)
#   OWN_BSSID                  — your own AP's 2.4 GHz BSSID (the only allowed target)
#   WS_URL                     — ws://<this-PC-LAN-IP>:8080/ws/node/node3
```

### 2. Getting `OWN_BSSID` — two hard rules

1. **2.4 GHz only.** The ESP32-S3 cannot use 5 GHz. A dual-band router has a
   *different* BSSID per band — you need the 2.4 GHz one. (macOS on a 5 GHz channel
   reports the 5 GHz BSSID, which is the wrong one.)
2. **Only an AP you own and are authorized to disrupt.** Use a **dedicated test AP**
   — a spare router or a 2.4 GHz phone hotspot — *not* shared corporate/office WiFi.
   Deauthing networks you don't own is illegal and disrupts others.

Ways to find your test AP's 2.4 GHz BSSID:

```bash
# From a Node1 capture of the beacons (SSID -> BSSID -> channel, 2.4G only):
./capture-node1.sh                       # a few seconds with the test AP on
cd ../backend && go run /tmp/beacons.go ../data/<newest>.pcap | grep -i "YourTestSSID"

# Or read it off the device (router admin page / phone hotspot MAC).
# Or unredact on macOS: enable Location Services for Terminal, then:
ipconfig getsummary en0 | grep BSSID
```

### 3. Flash + run

```bash
./flash-node3.sh                         # builds with your creds, flashes over USB

# Backend must allowlist the SAME BSSID and be reachable at WS_URL's IP:
cd ../backend && go run ./cmd/agent --own-bssids "AA:BB:CC:DD:EE:FF"
```

`WS_URL` must point at an IP the ESP32 can reach — put this PC and Node3 on the same
network. If you use a separate test AP, connect this PC to it and use its IP there.

### 4. Arm and fire (own AP only)

```bash
curl -X POST localhost:8080/api/safety/labmode -d '{"on":true}'
curl -X POST localhost:8080/api/nodes/node3/command \
  -d '{"cmd":"attack","type":"deauth","bssid":"AA:BB:CC:DD:EE:FF","confirm_own_net":true}'
```

Node3's green LED flashes bright on each transmitted attack; Node2 should raise a
`deauth_alert`, closing the attack → detect → record loop. Every TX is gated twice:
the backend `SafetyGate` (lab mode + `confirm_own_net` + BSSID allowlist) **and** the
firmware's compiled-in `OWN_BSSID`.

## Running the backend in Docker (optional)

The backend/UI/analysis ship as containers. For **real** Node3 testing the backend
must not run in `--demo` and must carry your allowlist — a gitignored
`docker-compose.override.yml` handles that:

```bash
echo 'OWN_BSSID=AA:BB:CC:DD:EE:FF' > ../.env     # repo-root .env
cd .. && docker compose up -d backend ui         # backend :8080, UI http://localhost:8088
```

Node3 then dials `ws://<host-LAN-IP>:8080/ws/node/node3` into the container. Don't run
a host `go run` agent and the container backend at once — they both bind `:8080`.

## Firmware build gotchas (for maintainers)

- **esp-idf-svc 0.51+** required — 0.49 hits a `c_char` u8/i8 bindings mismatch on the
  2025 esp/clang toolchain. Depend only on `esp-idf-svc` (it re-exports `::hal`/`::sys`).
- **`embuild`** needs `features = ["espidf"]` in `build.rs` deps.
- **Xtensa has no 64-bit atomics** — use `AtomicU32`/`Mutex`, not `AtomicU64`.
- **WS2812 LED** (Node2/3): the classic RMT API is behind `esp-idf-hal`'s `rmt-legacy`
  feature on ESP-IDF 5.x — `esp-idf-hal = { version = "0.45", features = ["rmt-legacy"] }`.
- **Node3 WebSocket client**: add the managed component and, because this is a *virtual*
  workspace, tell esp-idf-sys which crate holds the metadata:
  ```toml
  # node3-attacker/Cargo.toml
  [[package.metadata.esp-idf-sys.extra_components]]
  remote_component = { name = "espressif/esp_websocket_client", version = "1.2.0" }
  ```
  ```toml
  # node3-attacker/.cargo/config.toml [env]
  ESP_IDF_SYS_ROOT_CRATE = "node3-attacker"
  ```
  then `cargo clean -p esp-idf-sys` once to force the rebuild.
- **Deauth injection**: override `ieee80211_raw_frame_sanity_check` to return 0 AND add
  `-C link-arg=-Wl,--allow-multiple-definition` (else it collides with `libnet80211.a`).
