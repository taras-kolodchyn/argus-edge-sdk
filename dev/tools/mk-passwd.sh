#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
MOSQ_DIR="$(cd "$HERE/../mosquitto" && pwd)"

docker run --rm -v "$MOSQ_DIR":/mosquitto eclipse-mosquitto:2       mosquitto_passwd -b /mosquitto/passwords.txt "${MQTT_USERNAME:-devuser}" "${MQTT_PASSWORD:-devpass}"

echo "Created $MOSQ_DIR/passwords.txt for user ${MQTT_USERNAME:-devuser}"
