#!/usr/bin/env bash
# Run the Go agent against Node1 and report live capture stats. Use AFTER flashing
# and power-cycling the board so it is running the app (not in ROM download mode).
#
# Usage: ./firmware/capture-node1.sh [SERIAL_PORT]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${1:-$(ls /dev/cu.usbmodem* 2>/dev/null | head -n1 || true)}"
if [[ -z "$PORT" ]]; then
  echo "No /dev/cu.usbmodem* port found. Plug in the board." >&2
  exit 1
fi
echo "Capturing from $PORT -> PCAP + events in $REPO_ROOT/data"
echo "Ctrl-C to stop. (If packet count stays 0, the board is likely still in"
echo "download mode — unplug/replug the USB cable with BOOT not pressed.)"
cd "$REPO_ROOT/backend"
go run ./cmd/agent --node1-serial "$PORT" --baud 115200 --data "$REPO_ROOT/data"
