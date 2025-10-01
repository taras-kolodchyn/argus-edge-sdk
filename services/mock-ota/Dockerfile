# ---- chef ----
FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

# ---- planner ----
FROM chef AS planner
COPY services/ ./
RUN cargo chef prepare --recipe-path recipe.json

# ---- builder ----
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY services/ ./
RUN cargo build --release --bin mock-ota

# ---- runtime ----
FROM debian:bookworm-slim

RUN useradd -m app \
    && apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl procps libssl3 pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /home/app

COPY --from=builder /app/target/release/mock-ota /usr/local/bin/mock-ota
RUN chmod 0755 /usr/local/bin/mock-ota

USER app

ENV RUST_LOG=info \
    RUST_BACKTRACE=1 \
    MOCK_OTA_PORT=8090

HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=5 \
  CMD curl -fsS http://127.0.0.1:${MOCK_OTA_PORT}/healthz >/dev/null || exit 1

EXPOSE ${MOCK_OTA_PORT}

ENTRYPOINT ["/usr/local/bin/mock-ota"]
