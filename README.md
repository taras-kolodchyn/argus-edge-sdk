![Argus Edge SDK](/docs/argus_edge_sdk.jpg)
Open-source Edge SDK, firmware, and development kit for building, testing, and integrating IoT and sensor devices with the Gaia Project’s Argus service. Includes example firmware for Arduino/ESP32, a local MQTT/TLS development environment, OTA update examples, and backend mock services to enable community contributions and custom integrations.

[![CI][ci-badge]][ci-url]
[![Release][rel-badge]][rel-url]
[![Last commit][lc-badge]][lc-url]
[![Issues][issues-badge]][issues-url]
[![PRs][prs-badge]][prs-url]
[![Rust][rust-badge]][rust-url]
[![C++][cpp-badge]][cpp-url]
[![Arduino][arduino-badge]][arduino-url]
[![License][lic-badge]][lic-url]
[![Conventional Commits][cc-badge]][cc-url]
[![pre-commit][pc-badge]][pc-url]


## What's inside

- **services/** – Rust microservices (`mock-auth`, `mock-sink`, `mock-ota`) managed via a Cargo workspace.
- **deploy/compose/** – TLS-enabled Docker Compose stack, helper scripts, and environment templates.
- **firmware/** – example Arduino/ESP32 sketches and OTA `artifacts/` served to devices (stubs/placeholders for now).
- **docs/** – guides, topics, OTA examples, and architectural references.
- **Makefile** – common automation (`make dev-up`, `make dev-down`, `make test`, ...).

## Prerequisites

- Docker &amp; Docker Compose (v2+)
- (Optional) Mosquitto clients for quick testing: `mosquitto_pub`, `mosquitto_sub`
- (Optional) `jq` for parsing JSON responses in CLI examples
- Copy `deploy/compose/.env.example` to `.env` (or run `make dev-up`).

## Quick start (local stack)

1) **Clone &amp; enter the repo**
```bash
git clone https://github.com/taras-kolodchyn/argus-edge-sdk.git
cd argus-edge-sdk
```

2) **Bootstrap environment &amp; start the stack**
```bash
make dev-up
# or manually:
# cp deploy/compose/.env.example deploy/compose/.env
# (cd deploy/compose && docker compose up -d --build)
```

3) **Check health**
```bash
docker compose ps
curl -fsS http://localhost:8080/healthz 
# expected: ok
```

4) **Publish a test telemetry message**
```bash
docker compose exec mqtt sh -lc \
  'mosquitto_pub --cafile /certs/ca.crt -h "$MQTT_HOST" -p "$MQTT_PORT" \
    -u "$MQTT_USERNAME" -P "$MQTT_PASSWORD" \
    -t "${MQTT_TELEMETRY_TOPIC:-argus/devices/test}" \
    -m "{\\"temp\\":25,\\"pm25\\":10,\\"noise\\":42,\\"ts\\":123456789}"'
```
> Wildcards (`+`, `#`) are not allowed when publishing. Use a specific device topic (e.g. `argus/devices/test`). Wildcards can only be used when subscribing.
> Prefer to publish from your host? Copy the generated CA once (`docker compose cp mqtt:/certs/ca.crt ./dev-ca.crt`) and add `--cafile ./dev-ca.crt` when calling `mosquitto_pub` against `127.0.0.1:8883`.

5) **Watch the sink logs**
```bash
make dev-logs
# or: (cd deploy/compose && docker compose logs -f mock-sink)
```

6) **(Optional) Trigger a mock OTA rollout**
```bash
JOB_ID=$(curl -s -X POST http://localhost:8090/ota/jobs \
  -H 'Content-Type: application/json' \
  -d '{\"device_id\":\"device-123\",\"artifact\":\"mock-firmware.bin\",\"version\":\"1.0.1\"}' | jq -r '.id')

curl -s -X POST http://localhost:8090/ota/jobs/$JOB_ID/dispatch | jq
curl -s http://localhost:8090/ota/jobs/$JOB_ID | jq
```
> Follow the OTA flow with `make dev-logs SERVICE=mock-ota` (commands) and `make dev-logs SERVICE=mock-sink` / the device serial monitor for acknowledgements.

## Architecture

See [docs/architecture.md](docs/architecture.md) for a high-level overview of the repository layout and deployment workflow.

## Services

### mock-auth
- Small HTTP service used to simulate auth.
- Exposes **`/healthz`** on port **8080** (published to localhost:8080).
- Respects:
  - `RUST_LOG`, `RUST_BACKTRACE`
  - `MOCK_AUTH_ACCEPT_ANY_SECRET` (dev convenience)

#### Token flow and curl examples

Endpoints (all under `http://localhost:8080`):

- `POST /auth/device/register`
  - Request: `{ "device_id": "...", "pre_shared_secret": "..." }`
  - Response: `{ "device_id": "...", "token": "...", "mqtt_username": "...", "mqtt_password": "...", "expires_at": "RFC3339" }`
  - Notes: If `MOCK_AUTH_ACCEPT_ANY_SECRET=true` (default), any secret is accepted. If set to `false`, secrets shorter than 6 characters return `401`.

- `POST /auth/device/login`
  - Request: `{ "device_id": "...", "token": "..." }`
  - Response: `{ "access_token": "...", "expires_at": "RFC3339" }`
  - Notes: In the mock, any token with length >= 10 is accepted.

- `POST /auth/token/validate`
  - Request: `{ "access_token": "..." }`
  - Response: `{ "valid": true|false }`

- `GET /healthz` → `{ "status": "ok" }`

Request tracing:
- Every response includes header `X-Request-Id` (auto-generated UUID if absent on request).
- You can provide your own `X-Request-Id`; it will be propagated to response and appear in service logs to correlate requests.

Quick test with curl and jq:

```bash
# Health
curl -i http://localhost:8080/healthz

# 1) Register a device and capture token
DEVICE_ID=device-123
TOKEN=$(curl -s -X POST http://localhost:8080/auth/device/register \
  -H 'Content-Type: application/json' \
  -d "{\"device_id\":\"$DEVICE_ID\",\"pre_shared_secret\":\"testsecret\"}" | jq -r '.token')
echo "token=$TOKEN"

# 2) Login using the token from registration and capture access_token
ACCESS_TOKEN=$(curl -s -X POST http://localhost:8080/auth/device/login \
  -H 'Content-Type: application/json' \
  -d "{\"device_id\":\"$DEVICE_ID\",\"token\":\"$TOKEN\"}" | jq -r '.access_token')
echo "access_token=$ACCESS_TOKEN"

# 3) Validate the access token
curl -s -X POST http://localhost:8080/auth/token/validate \
  -H 'Content-Type: application/json' \
  -d "{\"access_token\":\"$ACCESS_TOKEN\"}" | jq

# Optional: Provide a custom request id and inspect it in response headers
curl -i -H 'X-Request-Id: demo-123' http://localhost:8080/healthz | sed -n '1,10p'
```

To tail logs and see request_id correlation:

```bash
make dev-logs SERVICE=mock-auth
# Example log lines include: request_id=... "device register request", "device login success", "token validate"
```

### mock-sink
- MQTT subscriber used for local testing.
- Subscribes to `MQTT_TOPICS` (default: `argus/devices/#`).
- Connects to broker using TLS (`MQTT_CA_PATH`, optional client certs) and `MQTT_URL`/`MQTT_HOST`/`MQTT_PORT`, `MQTT_USERNAME`, `MQTT_PASSWORD`.
- Logs parsed telemetry.

### mock-ota
- OTA control plane for dev. Exposes HTTP API on port **8090** (`/ota/jobs`, `/ota/artifacts`).
- Publishes commands to `argus/devices/{device_id}/ota` and listens for acknowledgements on `argus/devices/{device_id}/ota/status`.
- Serves files from `firmware/artifacts/` so devices can download mock firmware binaries.
- Sample flow:
  ```bash
  curl -s http://localhost:8090/ota/artifacts | jq
  JOB_ID=$(curl -s -X POST http://localhost:8090/ota/jobs \
    -H 'Content-Type: application/json' \
    -d '{\"device_id\":\"device-123\",\"artifact\":\"mock-firmware.bin\",\"version\":\"1.0.1\"}' | jq -r '.id')
  curl -s -X POST http://localhost:8090/ota/jobs/$JOB_ID/dispatch | jq
  ```

## Common workflows

**Rebuild just one service**
```bash
make dev-build
```

**Restart everything**
```bash
make dev-down
make dev-up
```

**Tail logs**
```bash
make dev-logs
# e.g. make dev-logs SERVICE=mock-ota
```

**Publish/subscribe with Mosquitto clients** (run from within `deploy/compose`)
```bash
# publish (inside container so CA is available)
docker compose exec mqtt sh -lc \
  'mosquitto_pub --cafile /certs/ca.crt -h "$MQTT_HOST" -p "$MQTT_PORT" \
    -u "$MQTT_USERNAME" -P "$MQTT_PASSWORD" \
    -t "${MQTT_TELEMETRY_TOPIC:-argus/devices/test}" \
    -m "{\\"temp\\":25,\\"pm25\\":10,\\"noise\\":42,\\"ts\\":123456789}"'

# subscribe (wildcard allowed)
docker compose exec mqtt sh -lc \
  'mosquitto_sub --cafile /certs/ca.crt -h "$MQTT_HOST" -p "$MQTT_PORT" \
    -u "$MQTT_USERNAME" -P "$MQTT_PASSWORD" \
    -t "${MQTT_TOPICS:-argus/devices/#}" -v'
```

**Manage OTA jobs**
```bash
curl -s http://localhost:8090/ota/jobs | jq
SERVICE_TOKEN=$(curl -s -X POST http://localhost:8080/auth/service/login \
  -H 'Content-Type: application/json' \
  -d '{"service":"mock-ota","secret":"ota-dev-secret"}' | jq -r '.access_token')
JOB_ID=$(curl -s -X POST http://localhost:8090/ota/jobs \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $SERVICE_TOKEN" \
  -d '{\"device_id\":\"device-123\",\"artifact\":\"mock-firmware.bin\",\"version\":\"1.0.1\"}' | jq -r '.id')
curl -s -X POST http://localhost:8090/ota/jobs/$JOB_ID/dispatch \
  -H "Authorization: Bearer $SERVICE_TOKEN" | jq
# simulate device ack (optional)
(cd deploy/compose && docker compose -f docker-compose.dev.yml exec mqtt sh -lc \
  'mosquitto_pub --cafile /certs/ca.crt -h "$MQTT_HOST" -p "$MQTT_PORT" \
    -u "$MQTT_USERNAME" -P "$MQTT_PASSWORD" \
    -t "${MQTT_TOPIC_PREFIX:-argus/devices/}device-123/ota/status" \
    -m "{\\\"job_id\\\":\\\"'$JOB_ID'\\\",\\\"status\\\":\\\"completed\\\",\\\"message\\\":\\\"manual ack\\\"}"')
curl -s http://localhost:8090/ota/jobs/$JOB_ID \
  -H "Authorization: Bearer $SERVICE_TOKEN" | jq
```

## Configuration (env)

| Variable | Description | Default |
|---|---|---|
| `MQTT_USERNAME` / `MQTT_PASSWORD` | Broker credentials | `devuser` / `devpass` |
| `MQTT_HOST`, `MQTT_PORT` | Broker host/port for in-cluster access (TLS) | `mqtt`, `8883` |
| `MQTT_URL` | Full broker URL. If set, overrides host/port. | `mqtt://mqtt:8883` |
| `MQTT_TOPIC_PREFIX` | Helpers for composing device topics | `argus/devices/` |
| `MQTT_TELEMETRY_TOPIC` | Default publish topic for helper scripts | `argus/devices/test` |
| `MQTT_TOPICS` | Topic filter(s) the sink subscribes to | `argus/devices/#` |
| `MQTT_CA_PATH` | CA certificate path used by mock-sink and scripts | `/certs/ca.crt` |
| `MOCK_OTA_HOST` | OTA service bind host | `0.0.0.0` |
| `MOCK_OTA_PORT` | OTA service bind port | `8090` |
| `MOCK_OTA_PUBLIC_BASE` | Base URL used in OTA commands | `http://mock-ota:8090` |
| `MOCK_OTA_ARTIFACT_DIR` | Path (inside container) to firmware artifacts | `/artifacts` |
| `RUST_LOG` | Log level for Rust services | `info` |
| `RUST_BACKTRACE` | Rust backtraces on panic | `1` |

> Copy `deploy/compose/.env.example` to `.env` for local use. Secrets should be managed via your preferred secret store in shared environments.

## Troubleshooting

- **Container keeps restarting or exits immediately**  
  Check env is loaded by Compose:
  ```bash
  (cd deploy/compose && docker compose config | awk '/env_file:/{p=1;next}/^[^[:space:]]/{p=0}p')
  (cd deploy/compose && docker compose exec mock-sink env | grep -E 'MQTT_|RUST_')
  ```
- **No messages in sink logs**  
  Ensure you publish to the topic in `MQTT_TOPICS` and credentials match those in `deploy/compose/.env`.
- **OTA job stuck in “dispatched”**  
  Check `make dev-logs SERVICE=mock-ota` (command side) and confirm the device firmware subscribed to `argus/devices/<device_id>/ota`.
- **Host vs. container addresses**  
  Inside containers use `mqtt:8883` (TLS). From your host use `127.0.0.1:8883` with `--cafile` pointing to the generated CA cert if you call `mosquitto_pub` directly.

## Contributing

See [`docs/getting-started.md`](./docs/getting-started.md) and [`docs/mqtt-topics.md`](./docs/mqtt-topics.md). Pull requests welcome!

## CI with act (local GitHub Actions runner)

You can run the CI workflow locally using [`act`](https://github.com/nektos/act), which emulates GitHub Actions jobs on your machine. This is useful for testing your workflow before pushing to GitHub.

To run the main CI job locally with your development environment variables, use:

```bash
act -j compose-smoke --env-file deploy/compose/.env.example \
  --container-architecture linux/amd64 \
  --bind \
  --container-options '--privileged --user root'

# run Arduino firmware lint/compile locally
act -W .github/workflows/arduino.yml -j lint \
  --container-architecture linux/amd64 \
  --secret GITHUB_TOKEN=${GITHUB_TOKEN:-ghp_dummy}
act -W .github/workflows/arduino.yml -j compile \
  --matrix '{"board":"arduino-uno"}' \
  --container-architecture linux/amd64 \
  --secret GITHUB_TOKEN=${GITHUB_TOKEN:-ghp_dummy}
act -W .github/workflows/arduino.yml -j compile \
  --matrix '{"board":"esp32-devkit"}' \
  --container-architecture linux/amd64 \
  --secret GITHUB_TOKEN=${GITHUB_TOKEN:-ghp_dummy}
act -W .github/workflows/arduino.yml -j compile \
  --matrix '{"board":"portenta-h7"}' \
  --container-architecture linux/amd64 \
  --secret GITHUB_TOKEN=${GITHUB_TOKEN:-ghp_dummy}

> **Note:** The first `compile` run pulls large toolchains (esp32 ≈600 MB,
> Portenta ≈400 MB). Expect the initial build to take several minutes while
> caches warm up; subsequent runs are much faster.
```

This command runs the `compose-smoke` job from the workflow, loading environment variables from `deploy/compose/.env`. Docker **must** be installed and available, as `act` will spin up containers to simulate the GitHub Actions CI environment.

**Advanced usage**

```bash
act -j compose-smoke \
  --env-file deploy/compose/.env.example \
  --container-architecture linux/amd64 \
  --bind \
  --container-options '--privileged --user root'
```
This variant ensures compatibility when jobs need privileged mode, root user, or volume binds.






[ci-badge]: https://github.com/taras-kolodchyn/argus-edge-sdk/actions/workflows/ci.yml/badge.svg?branch=main
[ci-url]: https://github.com/taras-kolodchyn/argus-edge-sdk/actions/workflows/ci.yml
[rel-badge]: https://img.shields.io/github/v/release/taras-kolodchyn/argus-edge-sdk?display_name=tag&sort=semver
[rel-url]: https://github.com/taras-kolodchyn/argus-edge-sdk/releases
[lc-badge]: https://img.shields.io/github/last-commit/taras-kolodchyn/argus-edge-sdk/main
[lc-url]: https://github.com/taras-kolodchyn/argus-edge-sdk/commits/main
[issues-badge]: https://img.shields.io/github/issues/taras-kolodchyn/argus-edge-sdk
[issues-url]: https://github.com/taras-kolodchyn/argus-edge-sdk/issues
[prs-badge]: https://img.shields.io/github/issues-pr/taras-kolodchyn/argus-edge-sdk
[prs-url]: https://github.com/taras-kolodchyn/argus-edge-sdk/pulls
[rust-badge]: https://img.shields.io/badge/Rust-1.80%2B-orange?logo=rust&logoColor=white
[rust-url]: https://www.rust-lang.org/
[lic-badge]: https://img.shields.io/badge/license-Apache--2.0-blue
[lic-url]: https://github.com/taras-kolodchyn/argus-edge-sdk/blob/main/LICENSE
[cc-badge]: https://img.shields.io/badge/Conventional%20Commits-1.0.0-yellow.svg
[cc-url]: https://conventionalcommits.org
[pc-badge]: https://img.shields.io/badge/pre--commit-enabled-brightgreen?logo=pre-commit&logoColor=white
[pc-url]: https://github.com/pre-commit/pre-commit
[cpp-badge]: https://img.shields.io/badge/C++-17-blue?logo=c%2B%2B&logoColor=white
[cpp-url]: https://isocpp.org/
[arduino-badge]: https://img.shields.io/badge/Arduino-IDE-00979D?logo=arduino&logoColor=white
[arduino-url]: https://www.arduino.cc/
