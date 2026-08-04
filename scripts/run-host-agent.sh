#!/usr/bin/env bash
# Run the Go agent on the Mac host so Node1 USB serial works.
# Docker Desktop on macOS cannot pass /dev/cu.usbmodem* into Linux containers.
#
#   1. Stops the containerized backend (frees :8080) and keeps it stopped
#   2. Recreates UI only (--no-deps) with host-gateway → Mac host agent
#   3. Starts host agent with --node1-serial + --own-bssids
#
# Node2/Node3 keep dialing ws://<this-Mac-LAN-IP>:8080/... as before.
# Restore containers afterwards:
#   docker compose up -d backend ui
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# shellcheck disable=SC1091
[[ -f .env ]] && set -a && source .env && set +a
OWN_BSSID="${OWN_BSSID:?set OWN_BSSID in .env (e.g. 90:3a:72:4d:0e:58)}"
export OWN_BSSID

PORT="${NODE1_SERIAL:-$(ls /dev/cu.usbmodem* 2>/dev/null | head -n1 || true)}"
if [[ -z "$PORT" ]]; then
  echo "No /dev/cu.usbmodem* found. Plug in Node1 (USB) first." >&2
  exit 1
fi
BAUD="${BAUD:-115200}"

echo "Stopping container backend (host agent will bind :8080)..."
docker compose stop backend >/dev/null
# Ensure restart policy does not bring it back while we run.
docker update --restart=no esp32_atk_def_proxy_logger-backend-1 >/dev/null 2>&1 || true

echo "Pointing UI at host agent (without starting backend)..."
docker compose -f docker-compose.yml -f docker-compose.override.yml \
  -f docker-compose.host-agent.yml up -d --no-deps ui >/dev/null

# Wait until :8080 is free (backend container fully released the publish).
for i in 1 2 3 4 5 6 7 8 9 10; do
  if ! lsof -nP -iTCP:8080 -sTCP:LISTEN >/dev/null 2>&1; then
    break
  fi
  echo "  waiting for :8080 to free ($i)..."
  # If backend somehow came back, stop it again.
  docker compose stop backend >/dev/null 2>&1 || true
  sleep 0.5
done
if lsof -nP -iTCP:8080 -sTCP:LISTEN >/dev/null 2>&1; then
  echo "Port :8080 still in use:" >&2
  lsof -nP -iTCP:8080 -sTCP:LISTEN >&2 || true
  echo "Stop whatever owns it, then re-run this script." >&2
  exit 1
fi

mkdir -p "$REPO_ROOT/data"
echo "Starting host agent:"
echo "  --node1-serial $PORT --baud $BAUD --own-bssids $OWN_BSSID"
echo "UI: http://localhost:8088   API: http://localhost:8080"
echo "Ctrl-C stops the agent; then: docker update --restart=unless-stopped esp32_atk_def_proxy_logger-backend-1 && docker compose up -d backend ui"
echo

cd "$REPO_ROOT/backend"
exec go run ./cmd/agent \
  --addr :8080 \
  --data "$REPO_ROOT/data" \
  --own-bssids "$OWN_BSSID" \
  --node1-serial "$PORT" \
  --baud "$BAUD"
