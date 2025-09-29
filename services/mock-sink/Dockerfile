# ---- chef ----
FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

# ---- planner ----
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY services services
RUN cargo chef prepare --recipe-path recipe.json

# ---- builder ----
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY services services
RUN cargo build --release --bin mock-sink

# ---- runtime ----
FROM debian:bookworm-slim

RUN useradd -m app \
    && apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates openssl procps \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /home/app

# install the compiled binary from the builder stage
COPY --from=builder /app/target/release/mock-sink /usr/local/bin/mock-sink
RUN chmod 0755 /usr/local/bin/mock-sink

# drop privileges
USER app

# runtime env
ENV RUST_LOG=info \
    RUST_BACKTRACE=1

# liveness/health check via process presence
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=5 \
  CMD pgrep -x mock-sink >/dev/null || exit 1

# run the service
ENTRYPOINT ["/usr/local/bin/mock-sink"]
