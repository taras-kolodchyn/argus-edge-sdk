#!/bin/sh
set -euo pipefail
IFS=$' \t\n'

SERVICE_NAME="${MOCK_OTA_SERVICE_NAME:-mock-ota}"
SERVICE_SECRET="${MOCK_OTA_SERVICE_SECRET:-ota-dev-secret}"
AUTH_HOST="${MOCK_AUTH_INTERNAL_HOST:-mock-auth}"
AUTH_PORT="${MOCK_AUTH_PORT:-8080}"
LOGIN_PATH="${MOCK_AUTH_SERVICE_LOGIN_PATH:-/auth/service/login}"
LOGIN_URL="${MOCK_AUTH_SERVICE_LOGIN_URL:-http://${AUTH_HOST}:${AUTH_PORT}${LOGIN_PATH}}"
SLEEP_SECONDS="${OTP_TEST_INTERVAL:-120}"
PUBLIC_BASE="${MOCK_OTA_PUBLIC_BASE:-http://mock-ota:8090}"
ARTIFACT_NAME="${OTP_TEST_ARTIFACT:-mock-firmware.bin}"
ARTIFACT_URL="${OTP_TEST_ARTIFACT_URL:-${PUBLIC_BASE%/}/ota/artifacts/${ARTIFACT_NAME}}"
ARTIFACT_SAVE_PATH="${OTP_TEST_ARTIFACT_SAVE_PATH:-}"

log() {
  printf '%s [otp-test] %s\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" "$1"
}

download_artifact() {
  local target cleanup http_code body_preview size label

  if [ -n "$ARTIFACT_SAVE_PATH" ]; then
    target="$ARTIFACT_SAVE_PATH"
    cleanup="false"
  else
    target=$(mktemp)
    cleanup="true"
  fi

  http_code=$(curl -sS -o "$target" -w '%{http_code}' "$ARTIFACT_URL" || printf '000')

  if [ "$http_code" = "200" ]; then
    size=$(wc -c < "$target" | awk '{print $1}' 2>/dev/null || printf '0')
    label="${ARTIFACT_NAME}"
    if [ "$cleanup" = "true" ]; then
      log "artifact download success (${label}, ${size:-0} bytes, stored=tmp)"
    else
      log "artifact download success (${label}, ${size:-0} bytes, stored=${target})"
    fi
  else
    body_preview=$(head -c 120 "$target" 2>/dev/null | tr '\n' ' ')
    log "artifact download failure (status=$http_code, url=${ARTIFACT_URL}, body=${body_preview:-<empty>})"
  fi

  if [ "$cleanup" = "true" ]; then
    rm -f "$target"
  fi
}

payload=$(printf '{"service":"%s","secret":"%s"}' "$SERVICE_NAME" "$SERVICE_SECRET")

while true; do
  log "requesting OTP token from ${LOGIN_URL}"

  status_file=$(mktemp)
  http_code=$(curl -sS -o "$status_file" -w '%{http_code}' \
    -X POST "$LOGIN_URL" \
    -H 'Content-Type: application/json' \
    -d "$payload" || printf '000')

  if [ "$http_code" = "200" ] && grep -q '"access_token"' "$status_file"; then
    short_token=$(grep -o '"access_token"\s*:\s*"[^"]\+"' "$status_file" | head -n1 | sed 's/.*"access_token"\s*:\s*"\([^"\n]\{0,6\}\).*/\1***/')
    log "success (${short_token:-token present})"
    download_artifact
  else
    body_preview=$(head -c 200 "$status_file" | tr '\n' ' ')
    log "failure (status=$http_code, body=${body_preview:-<empty>})"
  fi

  rm -f "$status_file"
  sleep "$SLEEP_SECONDS"
done
