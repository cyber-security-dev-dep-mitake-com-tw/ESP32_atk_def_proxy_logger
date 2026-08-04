# ESP32 Attack / Defense / Proxy Logger

A **headless**, PC-controlled 3-node ESP32 WiFi security toolkit. No physical
screen — all UI lives in the browser or terminal.

| Node  | Role                              | Transport        | Status |
|-------|----------------------------------|------------------|--------|
| Node1 | Packet Monitor + PCAP record     | USB serial (UART)| passive |
| Node2 | Deauth detector + alert          | WiFi WebSocket   | passive (STA + promiscuous on AP channel) |
| Node3 | Lab attack tester (own net only) | WiFi WebSocket   | **gated, off by default** — see [SAFETY.md](SAFETY.md) |

## Architecture

```
ESP32 nodes ──serial/WS──►  Go agent  ──REST+WS──►  React UI (browser)
                              │
                              ├─► data/*.pcap   (Node1 frames via gopacket)
                              └─► data/events.jsonl
                                        │
                                        └─►  Julia analysis  → reports/plots
```

All nodes speak one **NDJSON command/event protocol** (`backend/internal/proto`),
identical over serial and WebSocket, so the backend treats every node uniformly.

## Components

| Path        | Tech                        | What it is |
|-------------|-----------------------------|------------|
| `firmware/` | Rust (ESP-IDF) + host-tested `common` crate | Node firmwares; shared logic (802.11 parse, deauth detector, channel hopper, radiotap) |
| `backend/`  | Go (`net/http`, gopacket, gorilla/websocket, go.bug.st/serial) | Control agent: node transports, PCAP writer, event store, REST/WS API |
| `ui/`       | React + Vite + TypeScript   | Dashboard: node grid, packet stream, alert feed, gated attack console |
| `analysis/` | Julia                       | Offline PCAP/event analysis (channel use, RSSI, deauth timelines) |
| `tests/robot/` | Robot Framework          | End-to-end tests against the running agent |

## Quick start (no hardware — demo mode)

```bash
# 1. Backend with three synthetic nodes
cd backend && go build -o bin/agent ./cmd/agent
./bin/agent --demo --own-bssids "AA:BB:CC:DD:EE:FF"

# 2. UI (separate terminal) — proxies to the agent on :8080
cd ui && npm install && npm run dev   # http://localhost:5173

# 3. Analysis of captured events
cd analysis && julia --project=. run_report.jl ../data/events.jsonl
```

## Docker

The three PC-side services are containerized and wired together with Compose:

```bash
docker compose up backend ui          # backend :8080, UI at http://localhost:8088
docker compose --profile test run --rm robot   # E2E tests against the backend container
docker compose run --rm analysis \
  julia --project=. run_report.jl /data/events.jsonl   # one-shot report
```

| Service    | Image           | Port | Role |
|------------|-----------------|------|------|
| `backend`  | Go agent        | 8080 | REST/WS control API, PCAP + event store (shared `capture-data` volume) |
| `ui`       | nginx + React   | 8088 | dashboard; nginx proxies `/api` and `/ws` to `backend` |
| `analysis` | Julia           | —    | watches the shared volume and re-reports as events arrive |
| `robot`    | Robot Framework | —    | E2E suite; runs with `--profile test` against the running backend |

**How the other pieces reach the containers:**
- **Firmware Node2 / Node3 (WiFi)** dial `ws://<docker-host-LAN-IP>:8080/ws/node/node2`
  (resp. `node3`) — the backend port is published to the host, so nodes on the same
  network connect straight in.
- **Firmware Node1 (USB serial)** needs the adapter passed into the container:
  uncomment the `devices:` and `command:` overrides on the `backend` service in
  `docker-compose.yml` (e.g. `/dev/ttyUSB0`).
- **Robot Framework** runs either on the host (spawning its own agent) or as the
  `robot` container, which reaches the backend over the compose network at
  `http://backend:8080` (`START_AGENT:False`).

## With real hardware

```bash
# Node1 over serial (find the device with `ls /dev/tty.*` or `ls /dev/ttyUSB*`)
./bin/agent --node1-serial /dev/tty.usbserial-0001 --baud 921600 \
            --own-bssids "AA:BB:CC:DD:EE:FF"
```

Node2 and Node3 connect outbound over WiFi to `ws://<agent-host>:8080/ws/node/<id>`.
Flashing the firmware requires the esp Rust toolchain — see [firmware/README.md](firmware/README.md).

## Testing

```bash
cd backend  && go test -race ./...        # Go unit + API tests
cd firmware && cargo test -p common       # firmware logic (host)
cd analysis && julia --project=. test/runtests.jl
# End-to-end (from repo root, agent binary must be built first):
robot --outputdir tests/robot/output tests/robot/suites
```

CI runs all of the above on every push (`.github/workflows/ci.yml`); tagging
`vX.Y.Z` builds cross-platform agent binaries and a UI bundle into a GitHub Release
(`.github/workflows/release.yml`).

## REST API

| Method | Path                         | Purpose |
|--------|------------------------------|---------|
| GET    | `/api/health`                | liveness |
| GET    | `/api/nodes`                 | node statuses |
| POST   | `/api/nodes/{id}/command`    | send an NDJSON command (safety-gated) |
| GET    | `/api/safety`                | lab mode + allowlist |
| POST   | `/api/safety/labmode`        | `{"on": true}` |
| GET    | `/api/events`                | WebSocket: live event stream (browser) |
| GET    | `/ws/node/{id}`              | WebSocket: node dial-in (ESP32) |

See [SAFETY.md](SAFETY.md) before enabling Node3.
