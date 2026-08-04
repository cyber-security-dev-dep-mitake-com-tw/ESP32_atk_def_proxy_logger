#!/usr/bin/env bash
# Build and flash Node2 (WiFi deauth detector) to the connected ESP32-S3.
# WiFi credentials + backend WS URL are baked in from .wifi.env at build time.
#
# Usage:
#   1. cp firmware/node2-detector/.wifi.env.example firmware/node2-detector/.wifi.env
#   2. edit .wifi.env  (WIFI_SSID, WIFI_PASSWORD, WS_URL → .../ws/node/node2)
#   3. ./firmware/flash-node2.sh [SERIAL_PORT]
#
# After flashing, Node2 runs over WiFi — it dials the backend WS itself, no USB
# needed. Start the backend first:
#   docker compose up -d backend   # or: go run ./cmd/agent --own-bssids ...
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$REPO_ROOT/firmware/node2-detector/.wifi.env"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "Missing $ENV_FILE" >&2
  echo "Copy .wifi.env.example to .wifi.env and fill in your WiFi + WS_URL." >&2
  exit 1
fi
# shellcheck disable=SC1090
source "$ENV_FILE"
: "${WIFI_SSID:?set WIFI_SSID in .wifi.env}"
: "${WS_URL:?set WS_URL in .wifi.env}"
export WIFI_SSID WIFI_PASSWORD WS_URL
echo "Building Node2 for SSID='$WIFI_SSID'"
echo "Backend WS: $WS_URL"

# shellcheck disable=SC1090
source "$HOME/export-esp.sh"

cd "$REPO_ROOT/firmware/node2-detector"
cargo build --release
BIN="$REPO_ROOT/firmware/target/xtensa-esp32s3-espidf/release/node2-detector"

PORT="${1:-$(ls /dev/cu.usbmodem* 2>/dev/null | head -n1 || true)}"
if [[ -z "$PORT" ]]; then
  echo "No /dev/cu.usbmodem* port found." >&2
  echo "Put the board in download mode (hold BOOT, tap RESET, release BOOT) if needed." >&2
  exit 1
fi
echo "Flashing via $PORT"
espflash flash --after hard-reset --port "$PORT" "$BIN"

echo
echo "Flashed. Node2 will join WiFi and dial the backend at $WS_URL."
echo "Watch it appear:  curl -s localhost:8080/api/nodes"
echo "Blue LED = Node2 identity (bright flash on each deauth alert)."
