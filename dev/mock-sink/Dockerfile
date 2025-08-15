# ---- build ----
FROM rust:1-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main(){}" > src/main.rs && cargo build --release || true
COPY src ./src
RUN cargo build --release

# ---- runtime ----
FROM debian:bookworm-slim
RUN useradd -m app && apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
USER app
WORKDIR /home/app
COPY --from=build /app/target/release/mock-sink /usr/local/bin/mock-sink
ENV RUST_LOG=info
CMD ["mock-sink"]