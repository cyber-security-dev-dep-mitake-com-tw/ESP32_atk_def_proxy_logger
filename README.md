# ESP32 Attack / Defense / Proxy Logger

A **headless**, PC-controlled 3-node ESP32 WiFi security toolkit. No physical
screen — all UI lives in the browser or terminal.

| Node  | Role                              | Transport        | Status |
|-------|----------------------------------|------------------|--------|
| Node1 | Packet Monitor + PCAP record     | USB serial (UART)| passive |
| Node2 | Deauth detector + alert          | WiFi WebSocket   | passive |
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

---
一套 三節點 ESP32 WiFi 攻防／監控實驗台：板端抓包／偵測／（受限）攻擊，PC 端用 Go agent 匯流，瀏覽器儀表板控制與觀察，Julia 做離線分析。

## 目的

這個專案是一個**教學／實驗用途**的 WiFi 攻防實驗台，目標是在自己的實驗網路裡，
完整走一遍「監控 → 偵測 → 受控攻擊 → 記錄 → 離線分析」的閉環，讓使用者能：

- 用真實硬體（ESP32-S3）觀察 802.11 封包、頻道、RSSI 等底層資訊，而不只是模擬。
- 理解 deauthentication 攻擊的原理，同時看到偵測端如何即時發現異常。
- 練習「安全閘門」設計：攻擊功能預設關閉，且同時受後端 SafetyGate 與韌體端
  allowlist 雙重限制，只能對自己擁有、明確設定的 BSSID 動作（見 [SAFETY.md](SAFETY.md)）。
- 累積可回放的資料（PCAP、事件 JSONL），供 Julia 做離線統計與畫圖，而不是只看
  即時畫面。

**非目的**：這不是一個可以拿去打別人網路的攻擊工具，也不是一個要做成量產產品
的方案；三個節點刻意保持精簡（單一 SSID／單一 AP／單一攻擊向度），方便學習與
除錯，而不是覆蓋所有 802.11 攻防情境。

## 三個節點分別能做什麼

- **Node1（封包監控節點）**：把 ESP32-S3 的 WiFi 切到 promiscuous 模式，捕捉所有
  管理／資料 802.11 訊框，加上 radiotap 標頭後，透過 USB 序列埠即時串流給 PC。
  PC 端的 Go agent 會把這些訊框寫成標準 PCAP 檔，也能在儀表板上即時顯示頻道、
  RSSI、封包速率（pps）。它本身**只監聽、不主動發送**，是整個實驗台的「眼睛」。
  也接受 `set_channel` / `start_hop` 等指令切換或跳頻監控頻道。

- **Node2（deauth 偵測節點）**：以 station 身分連上自己的 AP，同時把射頻切到
  promiscuous（管理訊框）模式，在 AP 所在頻道上跑一個滑動視窗演算法
  （`common::detector`），偵測短時間內大量 deauth 訊框的異常模式。一旦超過門檻，
  就透過 WiFi WebSocket 送出 `deauth_alert` 事件給後端，並在板上用藍色 LED 標示
  身分。因為跳頻會讓已連線的 WS 斷線，預設**不主動跳頻**，維持在 AP 頻道上待命。

- **Node3（受限攻擊測試節點）**：同樣以 station 身分連上 WiFi、透過 WebSocket 聽
  後端指令，收到 `attack` 指令時才會組出並發送 802.11 deauth 訊框——但**只能**
  對編譯時寫死在韌體裡的 `OWN_BSSID` allowlist 目標動作。每一次發送都經過雙重
  閘門：後端 SafetyGate（lab mode + confirm_own_net + allowlist）先擋一次，韌體
  自己再擋一次不在允許清單內的 BSSID。板上綠色 LED 常駐心跳、攻擊瞬間閃亮綠燈
  標示狀態。**預設關閉，需刻意開啟才會作用**（見 [SAFETY.md](SAFETY.md)）。

## backend／analysis 能否部署到節點（Rust／C）上？

簡短結論：**不行，backend（Go）與 analysis（Julia）都無法整包搬到 ESP32 節點上
執行**，即使改寫成 Rust 或 C 也一樣，原因是它們依賴的是「有作業系統的 PC 環境」，
而不是 ESP32 這種資源受限的微控制器（MCU）：

- **backend（Go agent）**：依賴 `net/http`、`gorilla/websocket`、`go.bug.st/serial`
  等套件做 REST/WS 伺服器與序列埠讀寫，`gopacket` 更需要底層 libpcap／檔案系統
  來寫 PCAP 檔。這些都假設有完整 TCP/IP 協定堆疊、多執行緒排程、檔案系統與相對
  充裕的記憶體（GB 等級），而 ESP32-S3 只有約 512KB SRAM、無傳統檔案系統、且
  WiFi/藍牙協定堆疊已經占用大量資源。就算改寫成 Rust 或 C，只要邏輯本質仍是
  「多節點連線管理 + PCAP 檔案寫入 + REST/WS 伺服器」，就不適合塞進單一顆 MCU
  去同時服務所有節點——backend 的角色定位就是「PC 端匯流層」。

- **analysis（Julia）**：Julia 是一個帶 JIT 編譯器的完整語言執行環境，啟動即需要
  數十 MB 起跳的記憶體與磁碟空間，這遠超 ESP32 的能力範圍，無論用什麼語言重寫都
  一樣——分析所需的統計運算與畫圖，本質上就該放在有檔案系統與較多記憶體的機器
  上做離線批次處理，而不是即時跑在感測節點上。

- **真正可以搬到節點上的部分**：其實專案已經這樣做了——`firmware/common`
  這個 Rust crate 就是把「協定解析（802.11／radiotap）」與「deauth 偵測演算法」
  抽出來、寫成可在 host 上單元測試、也能直接編譯進 ESP-IDF 韌體的共用邏輯，
  Node2 用的偵測演算法就是這個 crate 的一部分。換句話說：**backend／analysis
  整體服務搬不上去，但它們裡面「純運算、無 OS 依賴」的邏輯（例如封包解析、
  簡單的統計視窗判斷）本來就可以、也應該用 Rust 或 C 直接寫進韌體**，讓節點端
  自己做輕量判斷，PC 端只保留匯流、儲存與離線深度分析的角色。

架構
```text
Node1 (USB serial) ──►┐
Node2 (WiFi WS)    ──►┼── Go agent (:8080) ── REST/WS ──► React UI (:8088)
Node3 (WiFi WS)    ──►┘         │
                                ├─ data/*.pcap
                                └─ data/events.jsonl ──► Julia analysis
```
三個節點
節點	角色	連線	現況
Node1
Promiscuous 抓 802.11 → PCAP／封包串流
USB serial（macOS 需 host agent）
UI上已連上：ch6、RSSI、pps
Node2
Deauth 偵測 → deauth_alert
WiFi WebSocket
已連上；藍燈
Node3
對 allowlist BSSID 打 deauth（雙重閘門）
WiFi WebSocket
已連上；綠燈；Lab console
PC 端已實現
Go agent：節點註冊、指令下發、事件 JSONL、Node1 PCAP、SafetyGate（lab mode + confirm + --own-bssids）
React UI：節點狀態、封包 monitor、deauth 警報／加總、Capture report（channel／RSSI／deauth）、攻擊控制台
Julia analysis：監看 events.jsonl，輸出與 UI Capture report 同類指標
Docker Compose：backend／UI／analysis；macOS 的 Node1 用 ./scripts/run-host-agent.sh（Docker 看不到 USB）
安全文件：SAFETY.md（僅自有網路、雙層 allowlist）

端到端已驗證過的閉環
Node1 有線抓包 → UI 顯示 ch／RSSI／pps
Node3 在 lab mode 對 90:3a:72:4d:0e:58 送 deauth
Node2 偵測並寫入 events／analysis
後端拒絕不在 allowlist／未確認的攻擊
刻意未做／限制
Node1 尚未改成純 WiFi（仍靠 USB；macOS 必須 host agent）
Node2 連上 AP 後預設不 hop channel（hop 會斷 WS）
攻擊僅限自有網路實驗，非法用途不在範圍內
簡言之：這是可操作的 WiFi 監控 + deauth 偵測 + 門控攻擊實驗 全棧，不是單一 firmware demo。

## 未來可做／值得投入的功能（Nice to have）

以下是目前刻意未做，但如果要往「更完整的實驗台」或「更接近真實產品」方向走，
值得考慮的功能，依大致優先順序排列：

**韌體 / 節點端**
- **Node1 改成純 WiFi 上傳**：目前仍依賴 USB serial（macOS 需額外跑
  `run-host-agent.sh`），若能像 Node2/Node3 一樣走 WebSocket，就能拿掉 host agent
  這層、簡化部署，也讓 Node1 可以真正無線佈署在遠端。
- **Node2 智慧跳頻**：目前為了保住 WS 連線，預設不 hop channel，只能盯著 AP
  所在頻道。可以研究「短暫跳出去掃一輪、再跳回來重連 WS」的排程方式，換取多
  頻道涵蓋範圍。
- **多 AP／多 BSSID 支援**：Node2 偵測與 Node3 allowlist 目前設計上偏向單一
  AP／單一目標，可以擴充成一次監控或保護多個 BSSID，貼近多 AP 的真實網路環境。
- **韌體端持久化設定**：`OWN_BSSID`／`WIFI_SSID` 等目前是編譯時寫死，之後可以
  改成從 NVS（ESP32 內建的非揮發性儲存）讀寫，換網路不用重新燒錄。
- **更多攻擊／防禦樣態**（僅限自有網路實驗）：例如 beacon flood 偵測、
  evil twin／rogue AP 偵測，讓 Node2 的偵測面向不只侷限在 deauth。

**後端 / 儀表板**
- **事件持久化到資料庫**：目前事件是 JSONL 檔案，之後量大或要跨多次實驗比對時，
  可以考慮寫入 SQLite／DuckDB，方便查詢與長期保存。
- **告警通知整合**：deauth_alert 觸發時，除了 UI 顯示，可以加上 webhook／email／
  Slack 通知，讓不盯著畫面也能知道異常。
- **多使用者／權限控管**：目前 REST API 沒有身分驗證，若要多人共用同一套實驗台，
  值得加上簡單的登入與角色（例如「只能看」vs「可以觸發攻擊」）。
- **歷史回放**：儀表板目前偏向即時畫面，之後可以加上「選擇一段時間範圍、重播
  當時的封包／事件」功能，方便事後分析與教學展示。

**分析 / 測試**
- **Julia 分析報告自動化輸出成 PDF／HTML**：目前輸出偏向文字／圖檔，可以整合
  成一份完整報告，方便存檔或分享。
- **CI 加上韌體端的靜態分析／lint**（`cargo clippy`）：目前 CI 已跑
  `cargo test -p common`，可以再加上 clippy 檢查韌體程式碼品質。
- **Robot Framework 測試涵蓋 Node3 攻擊閘門的邊界情境**：例如非 allowlist BSSID、
  lab mode 關閉時的行為，目前端到端測試多聚焦在正常閉環，可以補強異常路徑的
  回歸測試。

**文件 / 上手體驗**
- **一鍵環境檢查腳本**：偵測目前系統是否已安裝 esp Rust 工具鏈、Julia、Go、
  Docker 等相依，並提示缺少的部分，降低新人上手的摩擦。
- **教學情境腳本（scenario walkthrough）**：把「安全準備 → 開 lab mode → 觸發
  攻擊 → 觀察偵測 → 看分析報告」整理成一份手把手教學文件，作為課程或工作坊素材。

這些項目沒有強制排期，屬於「有餘力就做，能讓實驗台更好用／更完整」的清單，
與目前 [SAFETY.md](SAFETY.md) 所定義的安全邊界（僅自有網路、雙層 allowlist）
並不衝突——任何擴充都應該延續同樣的閘門設計原則。
