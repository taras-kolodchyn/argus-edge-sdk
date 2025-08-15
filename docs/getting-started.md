# Getting Started

This guide helps you run the local development environment and send a first telemetry payload.

## Requirements
- Docker & Docker Compose
- curl / Postman (optional)
- A board (Arduino/ESP32) or any MQTT client

## Quick Start
1. Generate MQTT credentials:
   ```bash
   cd dev/tools
   ./mk-passwd.sh
   ```
2. Start services:
   ```bash
   cd ..
   docker compose up --build -d
   ```
3. Register a device (mock):
   ```bash
   curl -s http://localhost:8080/auth/device/register          -H 'Content-Type: application/json'          -d '{"device_id":"device-123","pre_shared_secret":"abc123"}' | jq
   ```
4. Publish telemetry to topic `gaia/devices/device-123/telemetry`.
