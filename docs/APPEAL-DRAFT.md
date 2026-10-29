# Appeal draft

We request reconsideration of Soroban-Sentinel/sentinel-backend based on
substantive, published implementation work rather than a roadmap.

We added a working dynamic-analysis CLI and authenticated API that run real
Soroban native-host regression checks against an exact Git commit. Our included
vulnerable/fixed token pair reproduces a concrete self-transfer bug: a balance of
1 becomes 2 while total supply remains 1. Sentinel returns a failing CI exit code
and persists the violation and reproduction command for the vulnerable contract;
the fixed contract passes the same three checks through both the CLI and API.

We also added disposable source checkouts, repository/SHA validation, bearer
authentication, request and subprocess-output limits, process-group timeouts,
transactional findings, restart recovery and checked-in SQLx metadata. The
backend's 20 tests and strict formatting/lint checks pass locally and on GitHub
Actions. The hosted workflow also passed the vulnerable/fixed CLI and API
demonstration and uploaded machine-readable analysis evidence.

We corrected the scope of the project: these are real dynamic checks, not a claim
of automatic formal verification. Unsupported full verification fails explicitly;
contract-derived symbolic modelling and public multi-tenant execution remain
unimplemented. The current tool is intended for trusted Soroban development and
CI workspaces.

Implementation: https://github.com/Soroban-Sentinel/sentinel-backend/commit/b3ee084ed5fe677c2c656cb1d62479e6b1a8fbd0

Verification and reproducible results: https://github.com/Soroban-Sentinel/sentinel-backend/blob/main/docs/VERIFICATION.md

Detailed changes and limitations: https://github.com/Soroban-Sentinel/sentinel-backend/blob/main/docs/APPEAL-EVIDENCE.md

Passing hosted CI: https://github.com/Soroban-Sentinel/sentinel-backend/actions/runs/40869127981

Before submitting, confirm that the cited work postdates the rejection. This
draft has not been submitted and does not consume an appeal.
