#!/usr/bin/env bash
# Build and flash Node3 (lab attacker) to the connected ESP32-S3. WiFi credentials +
# the own-AP BSSID + backend WS URL are baked in from .wifi.env at build time.
#
# Usage:
#   1. cp firmware/node3-attacker/.wifi.env.example firmware/node3-attacker/.wifi.env
#   2. edit .wifi.env  (WIFI_SSID, WIFI_PASSWORD, OWN_BSSID, WS_URL)
#   3. ./firmware/flash-node3.sh [SERIAL_PORT]
#
# After flashing, Node3 runs over WiFi — it dials the backend WS itself, no USB
# needed. Start the backend first so it can accept the connection:
#   cd backend && go run ./cmd/agent   # listens on :8080, /ws/node/node3
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$REPO_ROOT/firmware/node3-attacker/.wifi.env"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "Missing $ENV_FILE" >&2
  echo "Copy .wifi.env.example to .wifi.env and fill in your WiFi + OWN_BSSID." >&2
  exit 1
fi
# shellcheck disable=SC1090
source "$ENV_FILE"
: "${WIFI_SSID:?set WIFI_SSID in .wifi.env}"
: "${OWN_BSSID:?set OWN_BSSID in .wifi.env}"
export WIFI_SSID WIFI_PASSWORD OWN_BSSID WS_URL
echo "Building Node3 for SSID='$WIFI_SSID', allowed target OWN_BSSID='$OWN_BSSID'"
echo "Backend WS: ${WS_URL:-<default>}"

# shellcheck disable=SC1090
source "$HOME/export-esp.sh"

cd "$REPO_ROOT/firmware/node3-attacker"
cargo build --release
BIN="$REPO_ROOT/firmware/target/xtensa-esp32s3-espidf/release/node3-attacker"

PORT="${1:-$(ls /dev/cu.usbmodem* 2>/dev/null | head -n1 || true)}"
if [[ -z "$PORT" ]]; then
  echo "No /dev/cu.usbmodem* port found." >&2
  exit 1
fi
echo "Flashing via $PORT"
# Default reset first (works on boards whose USB-JTAG auto-resets); if it can't
# connect, put the board in download mode (hold BOOT, tap RESET, release BOOT) and
# re-run.
espflash flash --after hard-reset --port "$PORT" "$BIN"

echo
echo "Flashed. Node3 will join WiFi and dial the backend at ${WS_URL:-the default}."
echo "Watch it appear:  curl -s localhost:8080/api/nodes"
echo "Green LED = Node3 identity (bright flash on each transmitted attack)."
