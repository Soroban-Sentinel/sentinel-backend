# sentinel-backend

A prototype backend for Soroban contract analysis: cargo-fuzz orchestration,
SQLite findings storage, an Axum API, and educational Z3 invariant templates.

## Current analysis guarantees

The worker requires an existing, reviewed contract-specific target at
`fuzz/fuzz_targets/<contract_name_with_underscores>.rs` and `fuzz/Cargo.toml`
in `SENTINEL_WORKSPACE_ROOT`. It does not overwrite targets with generated
placeholder code. Targets must actually invoke the Soroban contract client;
Sentinel cannot infer this from a filename.

Failed fuzz commands and missing harnesses produce `errored`, never `passed`.
Crash findings are preserved before formal analysis, with their artifact location.
`unique_edges` stores libFuzzer's last reported `cov:` count. The legacy
`coverage_pct` field is zero (unknown): the total reachable edge count is not
available, so no percentage is claimed.

**Formal contract verification is unavailable.** The Z3 equations are standalone
templates, not models extracted from contract code. `VerificationEngine::verify`
returns an error rather than claiming these templates prove a contract safe.
Consequently, the combined API pipeline currently finishes `errored` even when
fuzzing completes cleanly. Contract-derived modelling is required before the
combined pipeline can report a verified pass. Access control is also unsupported.
The harness generator remains an experimental scaffold, not a complete analyzer.

## Crates

| Crate | Role |
|---|---|
| `sentinel-core` | Shared domain types: `Run`, `Finding`, `SentinelConfig` |
| `sentinel-harness-gen` | Auto-generates `cargo-fuzz` harness source from contract config |
| `sentinel-fuzzer` | Spawns `cargo fuzz run`, parses crashes, aggregates coverage |
| `sentinel-verifier` | Educational Z3 templates; contract verification explicitly unavailable |
| `sentinel-api` | Axum REST API + SQLite persistence |

## Running locally

```bash
# Requires Rust, Python 3, libclang and Z3 development libraries.
# Prepare a disposable database for SQLx compile-time query checking.
python3 scripts/prepare-check-db.py
export SQLX_OFFLINE=false
export DATABASE_URL=sqlite:///tmp/sentinel-check.db

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
| `SENTINEL_WORKSPACE_ROOT` | `/tmp/sentinel-workspace` | Trusted local workspace containing reviewed fuzz targets |

### API error responses

API errors use a JSON object with an `error` message. `POST /runs` returns
`400 Bad Request` for invalid base64, non-UTF-8 or malformed TOML configuration,
and PR numbers larger than SQLite's signed 64-bit integer range. Requests for
individual runs or findings that do not exist return `404 Not Found`. Unexpected
server failures return `500 Internal Server Error` with a generic message;
internal details are recorded in server logs.

### Configuration validation

`POST /runs` rejects unsupported versions, empty or oversized contract lists,
unsafe names and paths, duplicate normalized targets, unknown or duplicate
invariants, and fuzz timeouts outside 1–3600 seconds before queuing work.
The worker repeats validation for callers outside the HTTP route.

The API currently assumes a trusted local workspace; it does not clone a repository
or check out the submitted commit. Authentication and per-run execution isolation
are not implemented. Deploy only in a trusted development environment until these
capabilities are implemented.

### Reproduce quality checks

After preparing the schema database and exporting the variables above:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
```

CI creates the same disposable schema from the checked-in migrations. No production
database or untracked SQLx cache is needed to build.
