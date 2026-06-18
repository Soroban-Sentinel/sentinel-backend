# ── Build stage ────────────────────────────────────────────────────────────────
FROM rust:1.79-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libz3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

RUN cargo build --release -p sentinel-api

# ── Runtime stage ──────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl3 \
    libz3-4 \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/sentinel-api ./sentinel-api
COPY --from=builder /app/crates/sentinel-api/migrations ./migrations

EXPOSE 8080
CMD ["./sentinel-api"]
