//! Spawns and supervises `cargo fuzz run` subprocesses.

use anyhow::{Context, Result};
use sentinel_core::{
    config::ContractConfig,
    finding::{Finding, FindingKind, FindingSeverity},
};
use std::{path::PathBuf, time::Duration};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{info, warn};
use uuid::Uuid;

pub struct FuzzRunner {
    /// Absolute path to the contract workspace root.
    pub workspace_root: PathBuf,
}

pub struct FuzzResult {
    pub findings: Vec<Finding>,
    /// Edge coverage percentage reported by libFuzzer (0–100).
    pub coverage_pct: f32,
    /// Total exec/s achieved.
    pub execs_per_sec: f64,
    pub unique_edges: u64,
}

impl FuzzRunner {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    /// Run the fuzzer for `contract` and return any findings.
    pub async fn run(&self, run_id: Uuid, contract: &ContractConfig) -> Result<FuzzResult> {
        let target_name = contract.name.replace('-', "_");
        let duration = Duration::from_secs(contract.fuzz_timeout_secs);

        info!(
            contract = %contract.name,
            timeout_secs = contract.fuzz_timeout_secs,
            "Starting fuzz run"
        );

        // cargo fuzz run <target> -- -max_total_time=<N> -print_final_stats=1
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&self.workspace_root)
            .args([
                "fuzz",
                "run",
                &target_name,
                "--",
                &format!("-max_total_time={}", contract.fuzz_timeout_secs),
                "-print_final_stats=1",
            ])
            .kill_on_drop(true);

        let output = timeout(duration + Duration::from_secs(30), cmd.output())
            .await
            .context("fuzz run timed out")?
            .context("cargo fuzz failed to spawn")?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let findings = self.parse_crashes(run_id, contract, &stderr);
        validate_exit(output.status.success(), &findings, &stderr)?;
        let coverage_pct = 0.0;
        let unique_edges = parse_edges(&stderr);
        let execs_per_sec = parse_execs(&stderr);

        info!(
            contract = %contract.name,
            findings = findings.len(),
            coverage_pct,
            "Fuzz run complete"
        );

        Ok(FuzzResult {
            findings,
            coverage_pct,
            execs_per_sec,
            unique_edges,
        })
    }

    /// Parse libFuzzer stderr for crash artifacts and turn them into Findings.
    fn parse_crashes(&self, run_id: Uuid, contract: &ContractConfig, stderr: &str) -> Vec<Finding> {
        let mut findings = Vec::new();

        for line in stderr.lines() {
            if line.contains("SUMMARY: libFuzzer: deadly signal")
                || line.contains("SUMMARY: libFuzzer: timeout")
                || line.contains("panicked at")
                || line.contains("ERROR: AddressSanitizer:")
                || line.contains("ERROR: libFuzzer:")
            {
                warn!(contract = %contract.name, line = %line, "Fuzz crash detected");
                findings.push(Finding::new(
                    run_id,
                    &contract.name,
                    FindingKind::FuzzCrash,
                    FindingSeverity::High,
                    format!("libFuzzer crash: {}", line.trim()),
                    extract_reproducer(stderr),
                ));
                break; // One process crash may produce several matching diagnostic lines.
            }
        }

        findings
    }
}

fn validate_exit(success: bool, findings: &[Finding], stderr: &str) -> Result<()> {
    anyhow::ensure!(
        success || !findings.is_empty(),
        "cargo fuzz failed without a recognized crash: {}",
        stderr.chars().take(2000).collect::<String>()
    );
    Ok(())
}

/// libFuzzer reports edge counts, not a percentage of all reachable edges.
fn parse_edges(stderr: &str) -> u64 {
    stderr
        .lines()
        .filter_map(|line| {
            let rest = line.split_once("cov:")?.1;
            rest.split_whitespace().next()?.parse().ok()
        })
        .next_back()
        .unwrap_or(0)
}

fn parse_execs(stderr: &str) -> f64 {
    for line in stderr.lines().rev() {
        if line.contains("exec/s:") {
            if let Some(part) = line.split("exec/s:").nth(1) {
                if let Ok(v) = part.split_whitespace().next().unwrap_or("0").parse::<f64>() {
                    return v;
                }
            }
        }
    }
    0.0
}

fn extract_reproducer(stderr: &str) -> Option<String> {
    for line in stderr.lines() {
        if line.contains("Test unit written to") {
            return Some(line.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_real_libfuzzer_progress() {
        let log = "#2 INITED cov: 12 ft: 13 corp: 1/1b exec/s: 0\n#800 DONE cov: 42 ft: 70 corp: 4/8b exec/s: 100";
        assert_eq!(parse_edges(log), 42);
        assert_eq!(parse_edges("error: no such command: fuzz"), 0);
    }
    #[test]
    fn tool_failures_are_not_clean_runs() {
        assert!(validate_exit(false, &[], "error: no such command: fuzz").is_err());
        assert!(validate_exit(true, &[], "").is_ok());
    }
    #[test]
    fn crash_is_deduplicated_and_keeps_reproducer() {
        let c = ContractConfig {
            name: "token".into(),
            path: "token".into(),
            invariants: vec![],
            fuzz_timeout_secs: 1,
            fuzz_iterations: 0,
        };
        let log = "thread panicked at token.rs:10\nSUMMARY: libFuzzer: deadly signal\nTest unit written to fuzz/artifacts/token/crash-123";
        let findings = FuzzRunner::new(".").parse_crashes(Uuid::new_v4(), &c, log);
        assert_eq!(findings.len(), 1);
        assert!(findings[0]
            .reproducer
            .as_ref()
            .unwrap()
            .contains("crash-123"));
        assert!(validate_exit(false, &findings, log).is_ok());
    }
    #[tokio::test]
    async fn missing_workspace_is_an_error() {
        let c = ContractConfig {
            name: "token".into(),
            path: "token".into(),
            invariants: vec![],
            fuzz_timeout_secs: 1,
            fuzz_iterations: 0,
        };
        assert!(FuzzRunner::new("/nonexistent-sentinel-workspace")
            .run(Uuid::new_v4(), &c)
            .await
            .is_err());
    }
}
