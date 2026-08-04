# ESP32 Attack / Defense / Proxy Logger

Headless, PC-controlled **3-node ESP32 WiFi** toolkit. No on-device screen — control and observation live in the browser or terminal.

> **Safety:** Node3 can transmit deauth frames. Use **only on networks you own**. Read [SAFETY.md](SAFETY.md) before enabling lab mode.

---

## Table of contents

- [Nodes](#nodes)
- [Architecture](#architecture)
- [Repository layout](#repository-layout)
- [Quick start (demo, no hardware)](#quick-start-demo-no-hardware)
- [Docker](#docker)
- [Real hardware](#real-hardware)
- [REST / WebSocket API](#rest--websocket-api)
- [Testing](#testing)
- [Related docs](#related-docs)

---

## Nodes

| Node | Role | Transport | Mode |
|------|------|-----------|------|
| **Node1** | Packet monitor + PCAP | USB serial | Passiveive |
| **Node2** | Deauth detector + alert | WiFi WebSocket | Passiveive (STA + promiscuous on AP channel) |
| **Node3** | Lab attack tester | WiFi WebSocket | **Gated / off by default** — [SAFETY.md](SAFETY.md) |

---

## Architecture

```text
ESP32 Node1 ──USB serial──┐
ESP32 Node2 ──WiFi WS─────┼──► Go agent (:8080) ──REST/WS──► React UI
ESP32 Node3 ──WiFi WS─────┘         │
                                    ├─► data/*.pcap
                                    └─► data/events.jsonl ──► Julia analysis
```

All nodes speak one **NDJSON** command/event protocol (`backend/internal/proto`) over serial or WebSocket, so the agent treats them uniformly.

---

## Repository layout

| Path | Stack | Purpose |
|------|--------|---------|
| [`firmware/`](firmware/) | Rust + ESP-IDF | Node firmwares; shared `common` crate (802.11 parse, detector, hopper, radiotap) |
| [`backend/`](backend/) | Go | Agent: transports, PCAP, event store, REST + WS API, safety gate |
| [`ui/`](ui/) | React + Vite + TS | Dashboard: nodes, packets, alerts, capture report, attack console |
| [`analysis/`](analysis/) | Julia | Offline / live rollups (channel, RSSI, deauth by BSSID) |
| [`tests/robot/`](tests/robot/) | Robot Framework | End-to-end tests against a running agent |
| [`scripts/`](scripts/) | Bash | Host helpers (e.g. macOS Node1 USB agent) |

---

## Quick start (demo, no hardware)

```bash
# 1) Backend with three synthetic nodes
cd backend
go build -o bin/agent ./cmd/agent
./bin/agent --demo --own-bssids "AA:BB:CC:DD:EE:FF"

# 2) UI (separate terminal) — proxies to :8080
cd ui
npm install
npm run dev          # http://localhost:5173

# 3) Analysis (optional)
cd analysis
julia --project=. run_report.jl ../data/events.jsonl
```

---

## Docker

```bash
# Core stack — API :8080, UI http://localhost:8088
docker compose up backend ui

# E2E tests
docker compose --profile test run --rm robot

# One-shot analysis report
docker compose run --rm analysis \
  julia --project=. run_report.jl /data/events.jsonl
```

### Services

| Service | Image role | Port | Notes |
|---------|------------|------|--------|
| `backend` | Go agent | **8080** | REST + WS; shared `capture-data` volume |
| `ui` | nginx + React | **8088** | Proxies `/api` and `/ws` → `backend` |
| `analysis` | Julia | — | Watches `/data/events.jsonl` |
| `robot` | Robot Framework | — | Profile `test` only |

### Reaching the stack

| Client | How it connects |
|--------|-----------------|
| **Node2 / Node3** | `ws://<host-LAN-IP>:8080/ws/node/node2` (or `node3`) |
| **Node1 (Linux Docker)** | Pass serial via `devices:` + `--node1-serial` in compose (see `docker-compose.yml` comments) |
| **Node1 (macOS)** | Docker Desktop **cannot** see `/dev/cu.usbmodem*`. Use [`scripts/run-host-agent.sh`](scripts/run-host-agent.sh) |
| **Robot (container)** | `http://backend:8080` with `START_AGENT:False` |

#### Real Node3 allowlist under Compose

```bash
echo 'OWN_BSSID=90:3a:72:4d:0e:58' > .env   # your own AP BSSID (2.4 GHz)
# docker-compose.override.yml (gitignored) should pass --own-bssids ${OWN_BSSID}
docker compose up -d backend ui
```

---

## Real hardware

### 1. Flash firmware

See [firmware/README.md](firmware/README.md).

```bash
./firmware/flash-node1.sh
./firmware/flash-node2.sh   # needs firmware/node2-detector/.wifi.env
./firmware/flash-node3.sh   # needs firmware/node3-attacker/.wifi.env
```

### 2. Run the agent

**Linux / generic (serial Node1):**

```bash
cd backend
./bin/agent \
  --node1-serial /dev/ttyUSB0 \
  --baud 115200 \
  --own-bssids "AA:BB:CC:DD:EE:FF"
```

**macOS (Node1 USB + WiFi Node2/3):**

```bash
echo 'OWN_BSSID=AA:BB:CC:DD:EE:FF' > .env
./scripts/run-host-agent.sh
# UI: http://localhost:8088
```

Node2 / Node3 dial outbound:

```text
ws://<agent-host-LAN-IP>:8080/ws/node/node2
ws://<agent-host-LAN-IP>:8080/ws/node/node3
```

### 3. Arm Node3 (own network only)

```bash
curl -s -X POST localhost:8080/api/safety/labmode \
  -H 'Content-Type: application/json' \
  -d '{"on":true}'

curl -s -X POST localhost:8080/api/nodes/node3/command \
  -H 'Content-Type: application/json' \
  -d '{"cmd":"attack","type":"deauth","bssid":"AA:BB:CC:DD:EE:FF","confirm_own_net":true}'
```

Backend allowlist (`--own-bssids`) and firmware `OWN_BSSID` must both match the **Wi‑Fi BSSID** (not the Mac private Wi‑Fi address, and not always the router LAN MAC).

---

## REST / WebSocket API

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/health` | Liveness |
| `GET` | `/api/nodes` | Node statuses |
| `POST` | `/api/nodes/{id}/command` | Send NDJSON command (safety-gated) |
| `GET` | `/api/safety` | Lab mode + allowlist |
| `POST` | `/api/safety/labmode` | Body: `{"on": true}` |
| `GET` | `/api/events` | WebSocket — live events (browser) |
| `GET` | `/ws/node/{id}` | WebSocket — ESP32 dial-in |

---

## Testing

```bash
cd backend  && go test -race ./...
cd firmware && cargo test -p common
cd analysis && julia --project=. test/runtests.jl

# E2E (agent binary built first)
robot --outputdir tests/robot/output tests/robot/suites
```

| Trigger | Workflow |
|---------|----------|
| Every push | [`.github/workflows/ci.yml`](.github/workflows/ci.yml) |
| Tag `vX.Y.Z` | [`.github/workflows/release.yml`](.github/workflows/release.yml) — agent binaries + UI bundle |

---

## Related docs

| Doc | Contents |
|-----|----------|
| [SAFETY.md](SAFETY.md) | Legal / safety gates for Node3 |
| [firmware/README.md](firmware/README.md) | Toolchain, flash, `.wifi.env`, LED identity |
