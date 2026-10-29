# Verification record

On 29 October 2026, implementation commit
[`b3ee084ed5fe677c2c656cb1d62479e6b1a8fbd0`](https://github.com/Soroban-Sentinel/sentinel-backend/commit/b3ee084ed5fe677c2c656cb1d62479e6b1a8fbd0)
was verified locally with Rust 1.96.0:

- Backend workspace: 20 tests passed.
- Rust formatting and strict Clippy checks passed.
- Vulnerable Soroban contract: CLI exit 1; authenticated API `failed`; one persisted
  balance-conservation finding. A supply-1 account ended with balance 2 after a
  self-transfer of 1.
- Fixed Soroban contract: CLI exit 0; authenticated API `passed`; no findings.
- Both variants executed all three requested Soroban checks through both entry
  points. Unauthorized API access returned 401, invalid base64 returned 400, and
  a mismatched source SHA returned 400.

[Measured result summary](VERIFICATION-RESULTS.json) records these observations
without transient local paths, process IDs or generated run UUIDs. Re-run
`python3 scripts/check-soroban-demo.py` for a complete fresh report at your current
Git HEAD. Passing regression tests do not constitute a formal correctness proof.

The [GitHub Actions implementation run](https://github.com/Soroban-Sentinel/sentinel-backend/actions/runs/40869127981) **passed**, including formatting, strict Clippy, all backend tests, both
contract variants through both entry points, and upload of the
`soroban-analysis-evidence` JSON artifact. This verifies the published checkout,
not only the local working directory.

Docker could not be built locally because Docker is not installed. The Dockerfile
was updated for the current toolchain, offline SQLx metadata and Cargo-based
runtime analysis, but no container-build result is claimed.
