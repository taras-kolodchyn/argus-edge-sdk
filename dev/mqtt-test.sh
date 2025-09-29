#!/bin/sh
set -eux

echo "[mqtt-test] Starting MQTT test..."

command -v mosquitto_pub
command -v mosquitto_sub

HOST="${MQTT_HOST:-mqtt}"
PORT="${MQTT_PORT:-8883}"
CA_PATH="${MQTT_CA_PATH:-}"
TLS_ARGS=""
if [ -n "$CA_PATH" ] && [ -r "$CA_PATH" ]; then
  TLS_ARGS="--cafile $CA_PATH"
fi

TOPIC="${MQTT_TELEMETRY_TOPIC:-gaia/devices/test}"

echo "[mqtt-test] hostname: $(hostname)"
echo "[mqtt-test] DNS check..."
(nslookup "$HOST" || ping -c1 "$HOST" || true)

echo "[mqtt-test] waiting for broker..."
for i in $(seq 1 30); do
  if mosquitto_pub $TLS_ARGS -h "$HOST" -p "$PORT" \
      -u "${MQTT_USERNAME:-devuser}" -P "${MQTT_PASSWORD:-devpass}" \
      -t hc -m ok >/dev/null 2>&1; then
    echo "[mqtt-test] broker is ready"
    break
  fi
  echo "[mqtt-test] still waiting ($i) ..."
  sleep 1
done

echo "[mqtt-test] Subscribing and publishing..."
( sleep 1; mosquitto_pub -d $TLS_ARGS -h "$HOST" -p "$PORT" \
    -u "${MQTT_USERNAME:-devuser}" -P "${MQTT_PASSWORD:-devpass}" \
    -t "$TOPIC" -m 'ok' ) &

mosquitto_sub $TLS_ARGS -h "$HOST" -p "$PORT" \
  -u "${MQTT_USERNAME:-devuser}" -P "${MQTT_PASSWORD:-devpass}" \
  -t "$TOPIC" -C 1 -W 10 -v \
  || { echo '[mqtt-test] FAIL (no message)'; exit 1; }

echo "[mqtt-test] OK"
echo "[mqtt-test] Done"
