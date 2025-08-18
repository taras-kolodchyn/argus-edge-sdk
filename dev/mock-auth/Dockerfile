# ---- chef ----
FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

# ---- planner ----
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

# ---- builder ----
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin mock-auth

# ---- runtime ----
FROM debian:bookworm-slim

RUN useradd -m app \
    && apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl procps \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /home/app

# install the compiled binary from the builder stage
COPY --from=builder /app/target/release/mock-auth /usr/local/bin/mock-auth
RUN chmod 0755 /usr/local/bin/mock-auth

# drop privileges
USER app

# runtime env
ENV RUST_LOG=info \
    RUST_BACKTRACE=1

# liveness/health check over HTTP
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=5 \
  CMD curl -fsS http://127.0.0.1:8080/healthz >/dev/null || exit 1

EXPOSE 8080

# run the server
ENTRYPOINT ["/usr/local/bin/mock-auth"]