# The service executes trusted Rust test packages, so its runtime includes Cargo.
FROM rust:1.96-slim-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --locked --release -p sentinel-api -p sentinel-fuzzer --bins

FROM rust:1.96-slim-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends git pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 sentinel \
    && mkdir /data \
    && chown sentinel:sentinel /data \
    && git config --system --add safe.directory /workspace
COPY --from=builder /app/target/release/sentinel-api /usr/local/bin/sentinel-api
COPY --from=builder /app/target/release/sentinel /usr/local/bin/sentinel
ENV SENTINEL_BIND_ADDR=0.0.0.0:8080 \
    SENTINEL_WORKSPACE_ROOT=/workspace \
    DATABASE_URL=sqlite:///data/sentinel.db \
    CARGO_HOME=/home/sentinel/.cargo
WORKDIR /data
USER sentinel
EXPOSE 8080
CMD ["sentinel-api"]
