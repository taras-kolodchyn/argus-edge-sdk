#!/bin/sh
set -eux

echo "[mqtt-test] Starting MQTT test..."

command -v mosquitto_pub
command -v mosquitto_sub

echo "[mqtt-test] hostname: $(hostname)"
echo "[mqtt-test] DNS check..."
(nslookup mqtt || ping -c1 mqtt || true)

echo "[mqtt-test] waiting for broker..."
for i in $(seq 1 30); do
  if mosquitto_pub -h mqtt -p 1883 \
      -u "${MQTT_USERNAME:-devuser}" -P "${MQTT_PASSWORD:-devpass}" \
      -t hc -m ok >/dev/null 2>&1; then
    echo "[mqtt-test] broker is ready"
    break
  fi
  echo "[mqtt-test] still waiting ($i) ..."
  sleep 1
done

echo "[mqtt-test] Subscribing and publishing..."
( sleep 1; mosquitto_pub -d -h mqtt -p 1883 \
    -u "${MQTT_USERNAME:-devuser}" -P "${MQTT_PASSWORD:-devpass}" \
    -t 'gaia/devices/selftest' -m 'ok' ) &

mosquitto_sub -h mqtt -p 1883 \
  -u "${MQTT_USERNAME:-devuser}" -P "${MQTT_PASSWORD:-devpass}" \
  -t 'gaia/devices/selftest' -C 1 -W 10 -v \
  || { echo '[mqtt-test] FAIL (no message)'; exit 1; }

echo "[mqtt-test] OK"
echo "[mqtt-test] Done"