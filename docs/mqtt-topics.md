# MQTT Topics (Dev Mock)

**Note:** These topics are for testing firmware with the local mock services; real production topics may differ.

- Telemetry publish (device → broker):
  - `gaia/devices/{device_id}`

- Optional status/heartbeat:
  - `gaia/devices/{device_id}/status`

- Command/control:
  - `gaia/devices/{device_id}/commands`

- OTA update:
  - `gaia/devices/{device_id}/ota`
