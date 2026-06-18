//! Spawns and supervises `cargo fuzz run` subprocesses.

use anyhow::{Context, Result};
use sentinel_core::{
    config::ContractConfig,
    finding::{Finding, FindingKind, FindingSeverity},
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
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
        let coverage_pct = parse_coverage(&stderr);
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
        })
    }

    /// Parse libFuzzer stderr for crash artifacts and turn them into Findings.
    fn parse_crashes(&self, run_id: Uuid, contract: &ContractConfig, stderr: &str) -> Vec<Finding> {
        let mut findings = Vec::new();

        for line in stderr.lines() {
            if line.contains("SUMMARY: libFuzzer: deadly signal")
                || line.contains("SUMMARY: libFuzzer: timeout")
                || line.contains("panicked at")
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
            }
        }

        findings
    }
}

/// Parse `cov: <N>` from libFuzzer stats line and return as percentage.
fn parse_coverage(stderr: &str) -> f32 {
    for line in stderr.lines() {
        if line.starts_with("stat::number_of_executed_units:") {
            // Simplified — real impl parses ft: edges
        }
        if let Some(rest) = line.strip_prefix("cov: ") {
            if let Ok(v) = rest.split_whitespace().next().unwrap_or("0").parse::<f32>() {
                return (v / 10000.0 * 100.0).min(100.0);
            }
        }
    }
    0.0
}

fn parse_execs(stderr: &str) -> f64 {
    for line in stderr.lines() {
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
