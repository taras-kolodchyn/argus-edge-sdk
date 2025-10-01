# Contributing

Thanks for your interest in **argus-edge-sdk**!  
The main focus of contributions is developing **firmware for devices** and covering as many sensors as possible.  
Mock services (`mock-auth`, `mock-sink`, `mock-ota`, Mosquitto) exist only for **local testing** of firmware and should not be extended with production features.

## Quick Start
1. **Fork** the repo and create a feature branch:
   ```bash
   git checkout -b feat/<short-topic>
   ```
2. Use the local dev stack for testing:
   ```bash
   make dev-up
   # or: (cd deploy/compose && cp -n .env.example .env && docker compose up -d)
   ```
3. (Optional) Send a test MQTT message:
   ```bash
   (cd deploy/compose && docker compose exec mqtt sh -lc \
     'mosquitto_pub --cafile /certs/ca.crt -h "$MQTT_HOST" -p "$MQTT_PORT" \
       -u "$MQTT_USERNAME" -P "$MQTT_PASSWORD" \
   -t "${MQTT_TELEMETRY_TOPIC:-argus/devices/test}" \
      -m "{\\"temp\\":25,\\"pm25\\":10,\\"noise\\":42,\\"ts\\":123456789}"')
   ```
3b. (Optional) Create and dispatch a mock OTA job:
   ```bash
   SERVICE_TOKEN=$(curl -s -X POST http://localhost:8080/auth/service/login \
     -H 'Content-Type: application/json' \
     -d '{"service":"mock-ota","secret":"ota-dev-secret"}' | jq -r '.access_token')
   (cd deploy/compose && JOB_ID=$(curl -s -X POST http://localhost:8090/ota/jobs \
     -H 'Content-Type: application/json' \
     -H "Authorization: Bearer $SERVICE_TOKEN" \
     -d '{"device_id":"device-123","artifact":"mock-firmware.bin","version":"1.0.1"}' | jq -r '.id') && \
     curl -s -X POST http://localhost:8090/ota/jobs/$JOB_ID/dispatch \
       -H "Authorization: Bearer $SERVICE_TOKEN" | jq)
   # optional: acknowledge via mqtt-client
   (cd deploy/compose && docker compose -f docker-compose.dev.yml exec mqtt sh -lc \
     'mosquitto_pub --cafile /certs/ca.crt -h "$MQTT_HOST" -p "$MQTT_PORT" \
       -u "$MQTT_USERNAME" -P "$MQTT_PASSWORD" \
      -t "${MQTT_TOPIC_PREFIX:-argus/devices/}device-123/ota/status" \
       -m "{\\"job_id\\":\\"'$JOB_ID'\\",\\"status\\":\\"completed\\",\\"message\\":\\"manual ack\\"}"')
   ```
4. Run the smoke tests locally with **act** (mirrors CI):
   ```bash
   act -j compose-smoke \
     --env-file deploy/compose/.env.example \
     --container-architecture linux/amd64 \
     --bind \
     --container-options '--privileged --user root'
   ```

## Project layout
- **/services** – Rust services (`mock-auth`, `mock-sink`, `mock-ota`)
- **/deploy/compose** – Docker Compose stack, env templates, helper scripts
- **/firmware** – device firmware and examples
  - `arduino/examples/` – minimal sketches (hello world, basic sensor usage)
  - `arduino/projects/` – more complete firmware combining multiple sensors
  - `arduino/lib/` – optional shared libraries, helpers, board configs
- **/docs** – documentation (you are here)

## Coding standards
- **Arduino**: keep examples self-contained, always include wiring diagrams
  ```bash
  arduino-cli compile --fqbn <board> firmware/arduino/examples/<example>
  ```

## Commit & PR guidelines
- Use **small, focused** commits. Prefer Conventional Commits style (`feat:`, `fix:`, `docs:`).
- Include examples/tests when changing firmware behavior.
- Update docs (`README.md`, `docs/`) when needed.
- PR checklist:
  - [ ] `docker compose up` works locally
  - [ ] Arduino code compiles with `arduino-cli`
  - [ ] formatters & linters pass (where applicable)
  - [ ] CI is green (`act -j compose-smoke` works locally)
  - [ ] no secrets / private endpoints in commits

## Secrets & security
- **Never** commit credentials or long-lived tokens.
- Use `secrets.h.template` for sensitive values (contributors should copy to `secrets.h` locally).
- `.env` is for local-only values. For CI, use GitHub Environments/Secrets.

## Issue triage
- Use labels: `bug`, `enhancement`, `good first issue`, `docs`, `infra`.
- When reporting a bug, include: repro steps, expected/actual behavior, logs, and SDK/firmware version.

## License
By contributing, you agree that your contributions are licensed under the project’s **Apache-2.0** license.
