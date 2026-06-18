# sentinel-backend

The Sentinel engine — fuzzing orchestration, Z3 formal verification, and REST API.

## Crates

| Crate | Role |
|---|---|
| `sentinel-core` | Shared domain types: `Run`, `Finding`, `SentinelConfig` |
| `sentinel-harness-gen` | Auto-generates `cargo-fuzz` harness source from contract config |
| `sentinel-fuzzer` | Spawns `cargo fuzz run`, parses crashes, aggregates coverage |
| `sentinel-verifier` | Z3 SMT engine — checks balance conservation, overflow, access control |
| `sentinel-api` | Axum REST API + SQLite persistence |

## Running locally

```bash
# Build everything
cargo build --workspace

# Start the API server (default: 0.0.0.0:8080)
RUST_LOG=info cargo run -p sentinel-api

# Run tests
cargo test --workspace
```

## API endpoints

| Method | Path | Description |
|---|---|---|
| `POST` | `/runs` | Trigger a new Sentinel run |
| `GET` | `/runs` | List recent runs |
| `GET` | `/runs/:id` | Get run details |
| `GET` | `/runs/:id/findings` | List findings for a run |
| `GET` | `/findings/:id` | Get a single finding |
| `GET` | `/coverage/:contract` | Historical coverage for a contract |

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | `sqlite://sentinel.db` | SQLite database path |
| `RUST_LOG` | `info` | Log level |
| `SENTINEL_API_TOKEN` | — | Bearer token for CI authentication |
