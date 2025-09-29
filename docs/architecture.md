# Architecture Overview

This repository hosts the Gaia Argus Edge SDK platform. It is organised into a few primary layers:

- **Services (`services/`):** Rust microservices that emulate the backend stack (`mock-auth` for registration/login, `mock-sink` for telemetry ingestion, `mock-ota` for firmware rollouts). Each service is an independent Cargo crate and Docker image, coordinated by the root workspace.
- **Deploy (`deploy/`):** Infrastructure-as-code assets. `deploy/compose` contains the TLS-enabled developer stack, helper scripts, and default environment templates. Additional deployment targets (e.g. Kubernetes) can live alongside it.
- **Firmware (`firmware/`):** Reference device sketches and SDK samples for Arduino/ESP32.
- **Docs (`docs/`):** Onboarding guides, OTA documentation, protocol references, and this architecture note.
- **Tooling (`Makefile`, `scripts/`):** Automation entry points for bootstrapping, building, and validating the stack.

## Local Workflow

1. Run `make dev-up` to bootstrap `.env`, build the containers, and start the Docker Compose stack.
2. Interact with the stack:
   - Publish telemetry through the `mqtt-client` sidecar or via the helper script (`deploy/compose/mqtt-test.sh`).
   - Create & dispatch OTA jobs with `curl` against `mock-ota` (see README for examples).
3. Inspect logs with `make dev-logs` or by targeting a specific service (`docker compose logs mock-sink`).
4. Shut the stack down with `make dev-down`.

The developer compose stack is fully TLS-enabled. Certificates are generated on first boot by the `init-mqtt` service and stored in the `argus-edge-sdk_certs` named volume.

## CI/CD

GitHub Actions runs two primary jobs:

- **Rust build/test:** builds the entire workspace (mock-auth, mock-sink, mock-ota) and runs crate-specific tests.
- **Docker Compose smoke test:** builds the service images, brings up the stack, publishes telemetry over TLS, dispatches a mock OTA job, simulates a device acknowledgment, and asserts that `mock-sink` consumes the message.

The workflow consumes the same `deploy/compose` assets as local developers, ensuring parity between environments. Future enterprise deployments can extend this folder with Kubernetes manifests or Helm charts without disrupting the local developer experience.

## Next Steps

- Package firmware builds in CI to provide downloadable device images per release.

