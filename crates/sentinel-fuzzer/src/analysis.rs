//! Explicitly scoped dynamic analysis. These results are not formal proofs.
use anyhow::{Context, Result};
use sentinel_core::{
    config::{AnalysisMode, ContractConfig},
    finding::{Finding, FindingKind, FindingSeverity},
};
use serde::Serialize;
use std::{path::Path, time::Duration};
use tokio::process::Command;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct AnalysisResult {
    pub contract: String,
    pub mode: AnalysisMode,
    pub findings: Vec<Finding>,
    pub checks_executed: u64,
    pub requested_invariants: Vec<String>,
    pub unique_edges: u64,
    pub coverage_pct: Option<f32>,
}

pub async fn analyze(
    root: &Path,
    run_id: Uuid,
    contract: &ContractConfig,
) -> Result<AnalysisResult> {
    anyhow::ensure!(contract.analysis != AnalysisMode::Full, "full verification unavailable: no contract-derived symbolic model; explicitly select fuzz or soroban_tests");
    let root = root.canonicalize().context("workspace not found")?;
    let path = root
        .join(&contract.path)
        .canonicalize()
        .context("contract path not found")?;
    anyhow::ensure!(path.starts_with(&root), "contract path escapes workspace");
    anyhow::ensure!(
        path.join("Cargo.toml").is_file(),
        "contract Cargo.toml not found"
    );
    if contract.analysis == AnalysisMode::Fuzz {
        let target = contract.name.replace('-', "_");
        anyhow::ensure!(
            root.join("fuzz/Cargo.toml").is_file(),
            "missing fuzz/Cargo.toml"
        );
        anyhow::ensure!(
            root.join("fuzz/fuzz_targets")
                .join(format!("{target}.rs"))
                .is_file(),
            "missing contract-specific fuzz target"
        );
        let result = crate::runner::FuzzRunner::new(&root)
            .run(run_id, contract)
            .await?;
        return Ok(AnalysisResult {
            contract: contract.name.clone(),
            mode: contract.analysis,
            findings: result.findings,
            checks_executed: 0,
            requested_invariants: contract.invariants.clone(),
            unique_edges: result.unique_edges,
            coverage_pct: None,
        });
    }
    let mut command = Command::new("cargo");
    command
        .env_remove("SENTINEL_API_TOKEN")
        .env_remove("DATABASE_URL")
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN");
    command.current_dir(&path).args([
        "test",
        "--locked",
        "--lib",
        "--color",
        "never",
        "sentinel_invariants::",
        "--",
        "--test-threads=1",
    ]);
    let output = crate::process::output(
        &mut command,
        Duration::from_secs(contract.fuzz_timeout_secs),
    )
    .await?;
    let log = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_tests(run_id, contract, output.status.success(), &log)
}

fn parse_tests(
    run_id: Uuid,
    contract: &ContractConfig,
    success: bool,
    log: &str,
) -> Result<AnalysisResult> {
    let mut findings = Vec::new();
    let mut checks = 0;
    for invariant in &contract.invariants {
        // Require the named result from Rust's captured test harness. Reject
        // ignored/missing tests and build failures rather than assuming success.
        let name = format!("sentinel_invariants::{invariant}");
        let passed = log
            .lines()
            .any(|line| line.trim() == format!("test {name} ... ok"));
        let failed = log.lines().any(|line| {
            line.trim() == format!("test {name} ... FAILED")
                || line.trim() == format!("{name} ... FAILED")
        });
        anyhow::ensure!(passed || failed, "required invariant test '{name}' did not produce a result; build failure, missing, ignored or malformed test output: {}", log.chars().rev().take(2000).collect::<String>().chars().rev().collect::<String>());
        checks += 1;
        if failed {
            let marker = format!("---- {name} stdout ----");
            let detail: String = log
                .split_once(&marker)
                .map(|(_, tail)| tail.split("\n---- ").next().unwrap_or(tail))
                .unwrap_or("")
                .chars()
                .take(2000)
                .collect();
            findings.push(Finding::new(run_id, &contract.name, FindingKind::InvariantViolation, FindingSeverity::High,
                format!("Soroban regression test failed: {name}. This is a dynamic test result, not a formal proof.\n{}", detail.trim()),
                Some(format!("cd {} && cargo test --locked --lib {name} -- --exact --nocapture", contract.path))));
        }
    }
    anyhow::ensure!(
        success || !findings.is_empty(),
        "test command failed outside the requested invariants"
    );
    anyhow::ensure!(
        !success || findings.is_empty(),
        "inconsistent test command results"
    );
    Ok(AnalysisResult {
        contract: contract.name.clone(),
        mode: contract.analysis,
        findings,
        checks_executed: checks,
        requested_invariants: contract.invariants.clone(),
        unique_edges: 0,
        coverage_pct: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> ContractConfig {
        ContractConfig {
            analysis: AnalysisMode::SorobanTests,
            name: "token".into(),
            path: "token".into(),
            invariants: vec!["balance_conservation".into()],
            fuzz_timeout_secs: 60,
            fuzz_iterations: 0,
        }
    }
    #[test]
    fn missing_ignored_and_build_failures_cannot_pass() {
        for log in [
            "running 0 tests",
            "error: could not compile",
            "test sentinel_invariants::balance_conservation ... ignored",
        ] {
            assert!(parse_tests(Uuid::new_v4(), &config(), true, log).is_err());
        }
    }
    #[test]
    fn records_real_test_results_and_reproducer() {
        let pass = parse_tests(
            Uuid::new_v4(),
            &config(),
            true,
            "test sentinel_invariants::balance_conservation ... ok",
        )
        .unwrap();
        assert_eq!(pass.checks_executed, 1);
        assert!(pass.findings.is_empty());
        let fail = parse_tests(
            Uuid::new_v4(),
            &config(),
            false,
            "test sentinel_invariants::balance_conservation ... FAILED",
        )
        .unwrap();
        assert_eq!(fail.findings.len(), 1);
        assert!(fail.findings[0]
            .reproducer
            .as_ref()
            .unwrap()
            .contains("--exact"));
    }
}
