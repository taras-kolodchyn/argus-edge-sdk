# MQTT Topics (Dev Mock)

**Note:** These topics are for testing firmware with the local mock services; real production topics may differ.

- Telemetry publish (device → broker):
  - `argus/devices/{device_id}/telemetry`

- Optional status/heartbeat:
  - `argus/devices/{device_id}/status`

- Command/control:
  - `argus/devices/{device_id}/commands`

- OTA update:
  - `argus/devices/{device_id}/ota`
