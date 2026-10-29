# Reconsideration evidence and remaining work

This records concrete local changes. It is not a claim of acceptance, production
readiness, or proof that these changes postdate the rejection. Link the published
commits and CI run, and establish that timeline before submitting an appeal.

## Implemented improvements

- Validate configuration before queueing or spawning work: supported version,
  bounded contract count and timeout, safe relative paths, safe target names,
  collision detection after hyphen normalization, and recognized invariants.
- Treat missing harnesses and failed fuzz commands as errors rather than clean
  runs. Record completion timestamps on pipeline errors.
- Require reviewed contract-specific targets and preserve their source instead
  of replacing them with generated placeholders.
- Preserve fuzz crash findings before attempting unsupported formal analysis;
  recognize sanitizer diagnostics, deduplicate crash messages, retain reproducer
  artifact locations, and persist observed edge counts.
- Remove fabricated coverage percentages. Zero in the legacy percentage field
  means unknown; use unique_edges for actual observed coverage.
- Separate educational SMT templates from contract verification. A contract
  without a derived symbolic model cannot receive a proof or a generic arithmetic
  counterexample attributed to its source. Bound the overflow template to signed
  i128 operands, test both overflow directions, and limit solver time.
- Make SQLx builds reproducible using a disposable database prepared from the
  migrations, and use that same setup in CI.
- Add regression tests for configuration rejection, failed tools, crash parsing,
  missing workspace/harness handling, completion records, and verification scope.

## Before spending a limited appeal

The strongest missing evidence is an end-to-end Soroban example that exercises
real contract methods and demonstrates a known vulnerable contract failing and a
fixed contract succeeding. The present combined pipeline intentionally errors
because contract-derived formal modelling is unavailable. Do not describe it as
complete formal verification or as production-ready CI enforcement.

Also implement authentication, isolate concurrent runs, bind execution to the
submitted repository and commit, and provide an immutable published CI result.
These are separate from the reliability fixes above.

## Draft factual appeal text

We have improved the reliability and transparency of sentinel-backend by adding
configuration validation, explicit failure handling for unavailable analysis,
regression tests, actual libFuzzer edge-count persistence, and reproducible SQLx
build checks. We removed misleading contract-proof and coverage claims and now
document the prototype's limitations. [Insert published commit links, passing CI,
and a real Soroban vulnerable/fixed demonstration before submission.]
