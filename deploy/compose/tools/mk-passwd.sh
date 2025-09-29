#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
MOSQ_DIR="$(cd "$HERE/../mosquitto" && pwd)"

# Ensure directory and empty file exist on host
mkdir -p "$MOSQ_DIR"
touch "$MOSQ_DIR/passwords.txt"

# Set permissions/ownership INSIDE the container, then create/update the password entry
# Using -u root to guarantee permission to chmod/chown in the container namespace

docker run --rm -u root -v "$MOSQ_DIR":/mosquitto eclipse-mosquitto:2 \
  sh -c "chmod 0700 /mosquitto/passwords.txt && \
         chown root:root /mosquitto/passwords.txt && \
         mosquitto_passwd -b /mosquitto/passwords.txt \"${MQTT_USERNAME:-devuser}\" \"${MQTT_PASSWORD:-devpass}\""

echo "Created $MOSQ_DIR/passwords.txt for user ${MQTT_USERNAME:-devuser}"