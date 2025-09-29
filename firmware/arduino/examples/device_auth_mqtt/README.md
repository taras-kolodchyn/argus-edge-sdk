ESP32 Arduino example: device register + login + MQTT publish

Prereqs
- Arduino IDE with ESP32 core installed
- Libraries: PubSubClient, ArduinoJson
- Local dev stack running (from repo root: `make dev-up`)

Configure
- Open `device_auth_mqtt.ino`
- Set `WIFI_SSID`, `WIFI_PASS`
- Set `AUTH_HOST` and `MQTT_HOST` to your computer’s LAN IP (not `localhost`)
- Optional: set a fixed `DEVICE_ID_CFG` (or leave empty to auto-generate)

Flash & Monitor
- Select your ESP32 board and port
- Upload the sketch
- Open Serial Monitor at 115200 baud

Expected flow
1) Connects to WiFi
2) POST /auth/device/register → token, mqtt_username, mqtt_password
3) POST /auth/device/login → access_token
4) Connect MQTT using credentials from register
5) Publishes telemetry every 5s to `gaia/devices/<device_id>`
6) Listens on `gaia/devices/<device_id>/ota` and sends status updates to `gaia/devices/<device_id>/ota/status` when an OTA job is dispatched

Troubleshooting
- If HTTP register/login fails, check `AUTH_HOST` resolves from the device network
- If MQTT doesn’t connect, ensure `MQTT_HOST` is reachable and mosquitto is running
- Tail service logs: `make dev-logs SERVICE=mock-sink` / `make dev-logs SERVICE=mock-ota`
