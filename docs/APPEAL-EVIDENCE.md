# Substantive improvement evidence

The previous implementation could report generic SMT equations as contract
verification and allow unavailable tooling to look like a clean run. The backend
now has a useful, explicit dynamic-analysis path and a real Soroban regression
fixture, with formal verification kept unavailable rather than overstated.

## What changed

| Area | Concrete improvement |
|---|---|
| Ecosystem relevance | Real Soroban SDK contracts, generated clients and native-host invocation, including a vulnerable/fixed self-transfer regression |
| Useful CI behavior | CLI exit 1 for observed violations, exit 0 for clean requested checks, exit 2 for incomplete execution; the API persists the same findings |
| Analysis scope | Explicit `soroban_tests`, `fuzz`, and fail-closed `full` modes; reports say dynamic analysis rather than formal proof |
| Provenance | Exact repository/commit validation, disposable Git clones without hard links, and source SHA stored in analysis reports |
| API protection | Required bearer credentials on all routes, loopback default, 64 KiB request limit, one analysis slot, and busy responses |
| Process supervision | Deadline and output caps; Unix process-group cleanup handles subprocess descendants |
| Findings and recovery | Transactional per-contract evidence, retained reproducer commands/inputs, error messages, completion timestamps and restart recovery |
| Correct measurements | Real observed edge counts; unknown percentages represented as null in reports |
| Build quality | Checked-in SQLx metadata, migration-backed validation, pinned contract dependencies, unused Wasmtime dependency removed |
| Repeatability | GitHub Actions runs the vulnerable/fixed demonstration through CLI and HTTP, asserting status and persisted findings, then uploads JSON evidence |

## Reproduce

From the repository root at the published commit:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace --bins
cargo fetch --locked --manifest-path examples/soroban-token/Cargo.toml
python3 scripts/check-soroban-demo.py
```

The demonstration's expected result is:

| Contract | CLI exit | API state | Persisted finding |
|---|---|---|---|
| Vulnerable | 1 | failed | Self-transfer breaks balance conservation |
| Fixed | 0 | passed | None across the three requested regression checks |

The vulnerable fixture starts with supply 1 and transfers 1 to the same account.
Its two writes target the same balance key and the final value becomes 2. The
fixed version recognizes the same-account case after authorization and amount
validation. Ordinary transfers, unauthorized transfers and arithmetic boundaries
are exercised through the same generated Soroban client in both fixtures.

The JSON report records the analyzed commit and scoped check counts. Consult the
published verification record alongside this document for results from the actual
run. The workflow source alone is not evidence that GitHub-hosted CI succeeded.

## Limits that remain explicit

These are developer-authored regression checks, not automatic discovery of every
vulnerability, a full Stellar token implementation, deployment tests or WASM-level
formal verification. Fuzz mode needs a reviewed contract-specific harness and
cargo-fuzz. Disposable Git clones isolate files but do not sandbox hostile build
scripts; this service is for trusted workspaces. Public multi-tenant hosting,
remote repository fetching, GitHub status publishing, multi-instance scheduling
and contract-derived symbolic modelling remain outside the implemented scope.

Do not assert that these commits postdate the rejection until the rejection date
is checked. Do not spend an appeal based on a promise of future work: use the
published changes and actual verification results.
