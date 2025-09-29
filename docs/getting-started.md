# Getting Started

This guide helps you run the local development environment and send a first telemetry payload.

## Requirements
- Docker & Docker Compose
- curl / Postman (optional)
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
