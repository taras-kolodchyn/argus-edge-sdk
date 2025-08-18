# argus-edge-sdk
Open-source Edge SDK, firmware, and development kit for building, testing, and integrating IoT and sensor devices with the Gaia Project’s Argus service. Includes example firmware for Arduino/ESP32, a local MQTT/TLS development environment, OTA update examples, and backend mock services to enable community contributions and custom integrations.

[![CI](https://github.com/taras-kolodchyn/argus-edge-sdk/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/taras-kolodchyn/argus-edge-sdk/actions/workflows/ci.yml)
![License](https://img.shields.io/badge/license-Apache--2.0-blue)

## What's inside

- **dev/** – local Docker Compose stack with Mosquitto (MQTT), `mock-auth`, and `mock-sink`
- **docs/** – guides, topics, OTA examples
- **firmware/** – example Arduino/ESP32 sketches (stubs/placeholders for now)

## Prerequisites

- Docker &amp; Docker Compose (v2+)
- (Optional) Mosquitto clients for quick testing: `mosquitto_pub`, `mosquitto_sub`
- The repo already includes a committed `.env` for local dev; no setup needed.

## Quick start (local stack)

1) **Clone &amp; enter the repo**
```bash
git clone https://github.com/taras-kolodchyn/argus-edge-sdk.git
cd argus-edge-sdk/dev
```

```
> **Note on `.env`**
> A ready-to-use `.env` file is already committed in this repo (for local development).
> You **do not** need to create `.env` or copy from an example. The committed values are
> intended for local testing only.
```

2) **Build and run everything**
```bash
docker compose up -d --build
# or force a fresh image rebuild:
# docker compose build --no-cache &amp;&amp; docker compose up -d
```

3) **Check health**
```bash
docker compose ps
curl -fsS http://localhost:8080/healthz 
# expected: ok
```

4) **Publish a test telemetry message**
> **Note**  
> If you run this directly in your shell, make sure environment variables are loaded:  
> ```bash
> set -a
> source .env
> set +a
> ```
> This ensures `$MQTT_USERNAME`, `$MQTT_PASSWORD`, and `$MQTT_TOPICS` are available.
```bash
mosquitto_pub -h 127.0.0.1 -p 1883 -u "$MQTT_USERNAME" -P "$MQTT_PASSWORD" \
  -t "argus/devices/device123/telemetry" \
  -m '{"temp":25,"pm25":10,"noise":42,"ts":123456789}'
```
> Wildcards (`+`, `#`) are not allowed when publishing. Use a specific device topic (e.g. `argus/devices/device123/telemetry`). Wildcards can only be used when subscribing.

5) **Watch the sink logs**
```bash
docker compose logs -f mock-sink
# you should see something like:
#  INFO mock_sink: $MQTT_TOPICS &lt;- {"temp":25,"pm25":10,"noise":42,"ts":123456789}
```

## Services

### mock-auth
- Small HTTP service used to simulate auth.
- Exposes **`/healthz`** on port **8080** (published to localhost:8080).
- Respects:
  - `RUST_LOG`, `RUST_BACKTRACE`
  - `MOCK_AUTH_ACCEPT_ANY_SECRET` (dev convenience)

### mock-sink
- MQTT subscriber used for local testing.
- Subscribes to `MQTT_TOPICS` (default: `argus/devices/+/telemetry`).
- Connects to broker using `MQTT_URL`/`MQTT_HOST`/`MQTT_PORT`, `MQTT_USERNAME`, `MQTT_PASSWORD`.
- Logs parsed telemetry.

## Common workflows

**Rebuild just one service**
```bash
docker compose build --no-cache mock-auth
docker compose up -d mock-auth
```

**Restart everything**
```bash
docker compose down -v
docker compose up -d --build
```

**Tail logs**
```bash
docker compose logs -f mqtt mock-auth mock-sink
```

**Publish/subscribe with Mosquitto clients**
```bash
# publish
mosquitto_pub -h 127.0.0.1 -p 1883 -u "$MQTT_USERNAME" -P "$MQTT_PASSWORD" \
  -t "argus/devices/device123/telemetry" \
  -m '{"temp":25,"pm25":10,"noise":42,"ts":123456789}'

# subscribe
mosquitto_sub -h 127.0.0.1 -p 1883 -u "$MQTT_USERNAME" -P "$MQTT_PASSWORD" \
  -t "argus/devices/#" -v
```

## Configuration (env)

| Variable | Description | Default |
|---|---|---|
| `MQTT_USERNAME` / `MQTT_PASSWORD` | Broker credentials | `devuser` / `devpass` |
| `MQTT_HOST`, `MQTT_PORT` | Broker host/port for in-cluster access | `mqtt`, `1883` |
| `MQTT_URL` | Full broker URL. If set, overrides host/port. | `mqtt://mqtt:1883` |
| `MQTT_TOPICS` | Topic filter(s) the sink subscribes to | `argus/devices/+/telemetry` |
| `RUST_LOG` | Log level for Rust services | `info` |
| `RUST_BACKTRACE` | Rust backtraces on panic | `1` |

## Troubleshooting

- **Container keeps restarting or exits immediately**  
  Check env is loaded by Compose:
  ```bash
  docker compose config | awk '/env_file:/{p=1;next}/^[^[:space:]]/{p=0}p'
  docker compose exec mock-sink env | grep -E 'MQTT_|RUST_'
  ```
- **No messages in sink logs**  
  Ensure you publish to the topic in `MQTT_TOPICS` and credentials match those in `.env`.
- **Host vs. container addresses**  
  Inside containers use `mqtt:1883`. From your host use `127.0.0.1:1883`.

## Contributing

See [`docs/getting-started.md`](./docs/getting-started.md) and [`docs/mqtt-topics.md`](./docs/mqtt-topics.md). Pull requests welcome!

## CI with act (local GitHub Actions runner)

You can run the CI workflow locally using [`act`](https://github.com/nektos/act), which emulates GitHub Actions jobs on your machine. This is useful for testing your workflow before pushing to GitHub.

To run the main CI job locally with your development environment variables, use:

```bash
act -j compose-smoke --env-file dev/.env
```

This command runs the `compose-smoke` job from the workflow, loading environment variables from your `.env` file. Docker **must** be installed and available, as `act` will spin up containers to simulate the GitHub Actions CI environment.

**Advanced usage**

```bash
act -j compose-smoke \
  --env-file dev/.env \
  --container-architecture linux/amd64 \
  --bind \
  --container-options '--privileged --user root'
```

This variant ensures compatibility when jobs need privileged mode, root user, or volume binds.  
