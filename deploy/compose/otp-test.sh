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

log() {
  printf '%s [otp-test] %s\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" "$1"
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
  else
    body_preview=$(head -c 200 "$status_file" | tr '\n' ' ')
    log "failure (status=$http_code, body=${body_preview:-<empty>})"
  fi

  rm -f "$status_file"
  sleep "$SLEEP_SECONDS"
done
