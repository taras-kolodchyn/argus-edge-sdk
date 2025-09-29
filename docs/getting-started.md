# Getting Started

This guide helps you run the local development environment and send a first telemetry payload.

## Requirements
- Docker & Docker Compose
- curl / Postman (optional)
- jq (optional, handy for CLI examples)
- A board (Arduino/ESP32) or any MQTT client

## Quick Start
1. Prepare your device:
   - If you are using Arduino/ESP32, open one of the firmware examples under `firmware/arduino/examples/`.
   - Update the WiFi/MQTT credentials in the sketch according to your environment.
   - Flash the firmware to your board.
2. Start services:
   ```bash
   make dev-up
   # or: (cd deploy/compose && cp -n .env.example .env && docker compose up --build -d)
   ```
3. Register a device (mock):
   ```bash
   curl -s http://localhost:8080/auth/device/register          -H 'Content-Type: application/json'          -d '{"device_id":"device-123","pre_shared_secret":"abc123"}' | jq
   ```
4. Publish telemetry to topic `gaia/devices/device-123`.
5. (Optional) Create an OTA job and dispatch it to the device.
   ```bash
   JOB_ID=$(curl -s -X POST http://localhost:8090/ota/jobs \
     -H 'Content-Type: application/json' \
     -d '{\"device_id\":\"device-123\",\"artifact\":\"mock-firmware.bin\",\"version\":\"1.0.1\"}' | jq -r '.id')
   curl -s -X POST http://localhost:8090/ota/jobs/$JOB_ID/dispatch | jq
   # (optional) simulate device ack
   (cd deploy/compose && docker compose -f docker-compose.dev.yml exec mqtt sh -lc \
     'mosquitto_pub --cafile /certs/ca.crt -h "$MQTT_HOST" -p "$MQTT_PORT" \
       -u "$MQTT_USERNAME" -P "$MQTT_PASSWORD" \
       -t "${MQTT_TOPIC_PREFIX:-gaia/devices/}device-123/ota/status" \
       -m "{\\\"job_id\\\":\\\"'$JOB_ID'\\\",\\\"status\\\":\\\"completed\\\",\\\"message\\\":\\\"manual ack\\\"}"')
   ```
   Watch `mock-ota` logs (`make dev-logs SERVICE=mock-ota`) and the device serial monitor to see the simulated firmware update.
