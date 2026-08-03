#!/usr/bin/env bash
# Build and flash Node1 to the connected ESP32-S3, then hand the serial port to the
# Go agent so captured frames land in a PCAP.
#
# Usage:
#   ./firmware/flash-node1.sh [DOWNLOAD_MODE_PORT]
#
# This S3 board exposes only its native USB, which does NOT auto-reset into the ROM
# bootloader, so you enter download mode by hand. In download mode the ROM presents a
# DIFFERENT serial device (e.g. /dev/cu.usbmodem1101) than the running app's console
# (/dev/cu.usbmodem<chip-id>) — so the port is detected AFTER you enter download mode.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OVERRIDE_PORT="${1:-}"

# esp Rust toolchain env.
# shellcheck disable=SC1090
source "$HOME/export-esp.sh"

cd "$REPO_ROOT/firmware/node1-monitor"
cargo build --release
BIN="$REPO_ROOT/firmware/node1-monitor/target/xtensa-esp32s3-espidf/release/node1-monitor"

cat <<'MSG'

  >>> Put the board in DOWNLOAD MODE now:
        1. Press and HOLD the BOOT button
        2. Briefly press and release RESET (may be labelled EN)
        3. Release BOOT
      Then press Enter here to flash.

MSG
read -r _

# Detect the download-mode port now (it appears when the ROM bootloader enumerates).
if [[ -n "$OVERRIDE_PORT" ]]; then
  PORT="$OVERRIDE_PORT"
else
  PORT="$(ls -t /dev/cu.usbmodem* 2>/dev/null | head -n1 || true)"
fi
if [[ -z "$PORT" ]]; then
  echo "No serial port found. Is the board in download mode?" >&2
  exit 1
fi
echo "Flashing via download-mode port: $PORT"

# --no-stub: flash straight from the ROM loader. The USB-Serial/JTAG re-enumerates
# when a stub is uploaded, which breaks the connection mid-flash, so we skip it.
# --before no-reset: the board is already in download mode.
# --after hard-reset: boot the freshly-flashed app afterwards.
espflash flash --no-stub --before no-reset --after hard-reset --port "$PORT" "$BIN"

# After the hard reset the app runs and its native-USB console re-enumerates under the
# chip-id name. Find it so we can tell the user which port to give the agent.
sleep 2
APP_PORT="$(ls /dev/cu.usbmodem* 2>/dev/null | grep -v "$(basename "$PORT")" | head -n1 || true)"
APP_PORT="${APP_PORT:-<your /dev/cu.usbmodem* port>}"

echo
echo "Flashed + reset. Node1 is now capturing. Record PCAP with the agent:"
echo "  cd $REPO_ROOT/backend && go run ./cmd/agent --node1-serial $APP_PORT --baud 115200"
