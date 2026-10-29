# Soroban Sentinel backend

Run repeatable dynamic checks against Soroban contracts and keep the findings,
reproduction commands, and exact source commit in a SQLite-backed API.

The included demonstration catches a real self-transfer balance-conservation bug:
the vulnerable token creates an extra token; the fixed token preserves supply.
Both versions execute the same three invariant tests through Soroban's native host
and generated contract client: balance conservation, authorization, and arithmetic
boundary handling. These are regression tests, not formal proofs or exhaustive
exploration of contract behavior.

## Try the real contract demonstration

Requires Rust 1.96.0, Git, Python 3, and (for the experimental verifier crate) Z3 and
Clang development libraries. On Debian/Ubuntu install `libz3-dev libclang-dev`.

```bash
cargo build --locked --workspace --bins
cargo fetch --locked --manifest-path examples/soroban-token/Cargo.toml
python3 scripts/check-soroban-demo.py
```

The script runs both contracts through the CLI **and the authenticated HTTP API**,
asserts that the vulnerable version fails with a persisted finding and the fixed
version passes, and saves JSON evidence to `/tmp/sentinel-soroban-evidence.json`.
It also checks unauthorized requests and rejection of a mismatched source SHA.
It analyzes tracked files at Git HEAD; commit local changes before running it.

To use only the CLI:

```bash
export CARGO_TARGET_DIR="$PWD/examples/soroban-token/target"
./target/debug/sentinel examples/vulnerable.sentinel.toml "$PWD" # exit 1: finding
./target/debug/sentinel examples/fixed.sentinel.toml "$PWD"      # exit 0: clean tests
```

CLI exit codes: `0` clean requested dynamic checks, `1` findings, `2` execution or
configuration error. Successful JSON reports include `source_commit`, analysis
mode, executed check count, findings and reproduction commands. The reproducer
runs from a checkout of that reported commit. No network RPC, wallet, funds or
Testnet account is required for this native host demonstration.

The pinned SDK and lockfile make the fixture repeatable. The fixture uses the
[official Soroban native contract testing API](https://docs.rs/soroban-sdk/22.0.11/soroban_sdk/struct.Env.html).
It is a small regression fixture, not an implementation of the complete Stellar
token interface or a deployable production token.

## Configure analysis

```toml
version = "1"

[[contracts]]
name = "fixed-token"
path = "examples/soroban-token/fixed"
analysis = "soroban_tests"
invariants = ["balance_conservation", "access_control", "no_overflow"]
fuzz_timeout_secs = 300
```

| Mode | Behavior |
|---|---|
| `soroban_tests` | Runs `cargo test --locked --lib sentinel_invariants::` in the contract package; requires an executed, non-ignored test for each declared invariant |
| `fuzz` | Runs an existing reviewed cargo-fuzz target named after the contract (hyphens become underscores); requires actual input execution or a recognized crash |
| `full` (default) | Errors explicitly: contract-derived symbolic verification is not implemented |

For `soroban_tests`, define tests named `sentinel_invariants::balance_conservation`,
`sentinel_invariants::access_control`, and/or `sentinel_invariants::no_overflow`.
Sentinel requires the tests named in the config; their assertions are authored and
reviewed by the contract developer. It does not infer a property from a test name.
Missing, ignored and unexecuted tests, compiler failures and timeouts cannot pass.
For `fuzz`, provide `fuzz/Cargo.toml` and `fuzz/fuzz_targets/<normalized_name>.rs`
at the workspace root. The harness must call the contract and assert the requested
invariants; automatic ABI-to-harness generation remains an experimental scaffold.
`fuzz_iterations` limits inputs in fuzz mode (zero leaves the time limit in control).
Install cargo-fuzz and its supported nightly toolchain separately for fuzz mode;
the Soroban regression demonstration does not require them.

Configuration validation rejects unknown fields/modes/invariants, unsafe names or
paths, normalized target collisions, duplicate invariants, empty or oversized
contract lists, and timeouts outside 1–3600 seconds. The timeout in test mode
includes compilation, so allow enough time on a cold build.

## Start the API

```bash
export SENTINEL_API_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
export SENTINEL_WORKSPACE_ROOT="$PWD"
export CARGO_TARGET_DIR="$PWD/examples/soroban-token/target"
cargo run --locked -p sentinel-api
```

Every endpoint requires `Authorization: Bearer <SENTINEL_API_TOKEN>`. The token must
be at least 32 printable ASCII bytes. Tokens are not forwarded to analysis
subprocesses. Requests are limited to 64 KiB; one job runs at a time and additional
submissions receive `503` with a retry message. Loopback binding is the default.

| Variable | Default | Purpose |
|---|---|---|
| `SENTINEL_API_TOKEN` | Required | Shared bearer credential for every route |
| `SENTINEL_WORKSPACE_ROOT` | `/tmp/sentinel-workspace` | Root of the trusted local Git repository |
| `SENTINEL_BIND_ADDR` | `127.0.0.1:8080` | API listening address |
| `DATABASE_URL` | `sqlite://sentinel.db` | Runtime SQLite database; created and migrated on startup |
| `CARGO_TARGET_DIR` | Cargo default | Optional absolute build cache shared between disposable runs |
| `RUST_LOG` | `sentinel_api=info` | Logging filter |

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/runs` | Submit `{repo, commit_sha, config_b64, pr_number?}` |
| `GET` | `/runs` | List 50 recent runs |
| `GET` | `/runs/:id` | Status, scoped analysis report and execution error |
| `GET` | `/runs/:id/findings` | Persisted findings and reproducers |
| `GET` | `/findings/:id` | One finding |
| `GET` | `/coverage/:contract` | Historical observed fuzz edge counts |

`repo` must match the configured workspace's `owner/repository` origin and
`commit_sha` must match its full HEAD SHA. Each job clones tracked Git objects
locally without hard links and checks out the exact requested commit in a temporary
directory. Uncommitted changes and untracked files are not analyzed. No caller
supplied repository is fetched. The clone is removed when the job ends. Findings
and reports are stored transactionally per contract; prior evidence survives an
error in a later contract. On restart unfinished jobs become completed errors.

`passed` means the explicitly requested dynamic checks completed without findings;
`failed` means findings were observed; `errored` means execution was incomplete.
Neither `passed` nor the experimental Z3 templates establish formal correctness.
`coverage_pct` in run reports is `null` because total reachable edges are unknown.
For historical compatibility, coverage snapshots use zero for unknown percentages;
use `unique_edges` for actual observed fuzz coverage. Fuzz inputs up to 64 KiB are
stored as hex JSON so they survive temporary checkout deletion.

API errors are JSON: `400` for invalid submissions, `401` for missing/invalid
credentials, `404` for absent records, `503` when busy, and generic `500` for
unexpected server failures. Execution failures are recorded in `error_message` on
the run instead of being represented as successful checks.

## Development and CI

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --workspace
```

Checked-in `.sqlx` query metadata supports building without a live database. To
validate changed SQL against migrations and refresh that metadata:

```bash
python3 scripts/prepare-check-db.py
SQLX_OFFLINE=false DATABASE_URL=sqlite:///tmp/sentinel-check.db \
  SQLX_OFFLINE_DIR="$PWD/.sqlx" cargo check --workspace
```

GitHub Actions runs formatting, linting, backend tests, and the vulnerable/fixed
CLI/API demonstration. It uploads the JSON analysis evidence as an artifact.
The unused Wasmtime dependency was removed: dynamic Soroban tests use the SDK's
host, and fuzz execution uses cargo-fuzz rather than an embedded Wasmtime runtime.

## Scope and deployment boundary

This is a development/CI tool for trusted contract workspaces. A disposable source
checkout isolates files and pins the revision; it is not an OS security sandbox.
Cargo build scripts and tests execute code with the service user's privileges.
Use a dedicated container or machine for your trusted repositories. Do not expose
this service to arbitrary untrusted source. It currently has one shared bearer
credential, one job slot, and no multi-instance coordination, GitHub webhook
integration, merge-status publishing, remote checkout service or WASM-level proof
engine. Public/hostile multi-tenant hosting needs separate sandboxing and tenancy.

`sentinel-verifier` contains educational bounded Z3 templates, and
`sentinel-harness-gen` contains experimental harness scaffolds. Neither is presented
as automatic verification of arbitrary Soroban code.

See [appeal evidence](docs/APPEAL-EVIDENCE.md) for the changes and remaining limits.
