//! Background job processor — executes a queued Sentinel run end-to-end.
//!
//! Flow per contract:
//!   1. Write fuzz harness via sentinel-harness-gen
//!   2. Run cargo-fuzz subprocess via sentinel-fuzzer (skipped gracefully if unavailable)
//!   3. Run Z3 invariant checks via sentinel-verifier
//!   4. Persist findings + coverage snapshot to SQLite
//! Finally updates the run row with final status, coverage %, and finding count.

use anyhow::Result;
use sentinel_core::{
    config::SentinelConfig,
    finding::{Finding, FindingKind, FindingSeverity},
};
use sentinel_fuzzer::runner::FuzzRunner;
use sentinel_verifier::engine::VerificationEngine;
use sqlx::SqlitePool;
use std::path::PathBuf;
use tracing::{error, info, warn};
use uuid::Uuid;

pub async fn execute_run(
    db: SqlitePool,
    run_id: String,
    config: SentinelConfig,
    workspace_root: PathBuf,
) {
    info!(run_id = %run_id, contracts = config.contracts.len(), "Background run starting");

    if let Err(e) = set_status(&db, &run_id, "running").await {
        error!(run_id = %run_id, error = %e, "Failed to set run status to running");
        return;
    }

    match run_pipeline(&db, &run_id, &config, &workspace_root).await {
        Ok((finding_count, coverage_pct)) => {
            let status = if finding_count > 0 { "failed" } else { "passed" };
            if let Err(e) = finish_run(&db, &run_id, status, finding_count, coverage_pct).await {
                error!(run_id = %run_id, error = %e, "Failed to finalize run record");
            } else {
                info!(run_id = %run_id, status, finding_count, coverage_pct, "Run complete");
            }
        }
        Err(e) => {
            error!(run_id = %run_id, error = %e, "Run pipeline errored");
            let _ = set_status(&db, &run_id, "errored").await;
        }
    }
}

async fn run_pipeline(
    db: &SqlitePool,
    run_id: &str,
    config: &SentinelConfig,
    workspace_root: &PathBuf,
) -> Result<(i64, f64)> {
    let run_uuid = Uuid::parse_str(run_id)?;
    let mut total_findings: i64 = 0;
    let mut total_coverage: f64 = 0.0;

    for contract in &config.contracts {
        info!(run_id = %run_id, contract = %contract.name, "Processing contract");

        // ── 1. Harness generation ──────────────────────────────────────────
        match sentinel_harness_gen::generate_harness(contract) {
            Ok(harness_src) => {
                let fuzz_dir = workspace_root.join("fuzz").join("fuzz_targets");
                match tokio::fs::create_dir_all(&fuzz_dir).await {
                    Err(e) => warn!(run_id = %run_id, error = %e, "Could not create fuzz target dir"),
                    Ok(_) => {
                        let target_name = contract.name.replace('-', "_");
                        let harness_path = fuzz_dir.join(format!("{}.rs", target_name));
                        match tokio::fs::write(&harness_path, &harness_src).await {
                            Err(e) => warn!(run_id = %run_id, error = %e, "Could not write harness file"),
                            Ok(_) => info!(run_id = %run_id, path = %harness_path.display(), "Harness written"),
                        }
                    }
                }
            }
            Err(e) => warn!(run_id = %run_id, contract = %contract.name, error = %e, "Harness generation failed"),
        }

        // ── 2. Fuzzing ─────────────────────────────────────────────────────
        let fuzz_findings = {
            let fuzzer = FuzzRunner::new(workspace_root);
            match fuzzer.run(run_uuid, contract).await {
                Ok(result) => {
                    total_coverage += result.coverage_pct as f64;
                    let cov = result.coverage_pct as f64;
                    let snap_at = chrono::Utc::now().to_rfc3339();
                    let edges: i64 = 0;
                    sqlx::query!(
                        "INSERT INTO coverage_snapshots (run_id, contract_name, coverage_pct, unique_edges, snapshot_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        run_id, contract.name, cov, edges, snap_at
                    )
                    .execute(db)
                    .await?;
                    result.findings
                }
                Err(e) => {
                    warn!(
                        run_id = %run_id,
                        contract = %contract.name,
                        error = %e,
                        "Fuzzer unavailable or failed — recording zero coverage"
                    );
                    let snap_at = chrono::Utc::now().to_rfc3339();
                    let edges: i64 = 0;
                    let cov: f64 = 0.0;
                    // Non-fatal: insert a zero-coverage snapshot so the dashboard shows history
                    let _ = sqlx::query!(
                        "INSERT INTO coverage_snapshots (run_id, contract_name, coverage_pct, unique_edges, snapshot_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        run_id, contract.name, cov, edges, snap_at
                    )
                    .execute(db)
                    .await;
                    vec![]
                }
            }
        };

        // ── 3. Formal verification ─────────────────────────────────────────
        let verif_findings = match VerificationEngine::verify(run_uuid, contract) {
            Ok(f) => f,
            Err(e) => {
                warn!(run_id = %run_id, contract = %contract.name, error = %e, "Verifier error");
                vec![]
            }
        };

        // ── 4. Persist findings ────────────────────────────────────────────
        let all_findings: Vec<Finding> = fuzz_findings.into_iter().chain(verif_findings).collect();
        total_findings += all_findings.len() as i64;

        for finding in all_findings {
            let fid = finding.id.to_string();
            let rid = finding.run_id.to_string();
            let kind = kind_str(&finding.kind);
            let severity = severity_str(&finding.severity);
            let created_at = finding.created_at.to_rfc3339();
            sqlx::query!(
                "INSERT INTO findings (id, run_id, contract_name, kind, severity, description, reproducer, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                fid, rid, finding.contract_name, kind, severity, finding.description, finding.reproducer, created_at
            )
            .execute(db)
            .await?;
        }
    }

    let contract_count = config.contracts.len().max(1) as f64;
    let avg_coverage = total_coverage / contract_count;

    Ok((total_findings, avg_coverage))
}

async fn set_status(db: &SqlitePool, run_id: &str, status: &str) -> Result<()> {
    sqlx::query!("UPDATE runs SET status = ?1 WHERE id = ?2", status, run_id)
        .execute(db)
        .await?;
    Ok(())
}

async fn finish_run(
    db: &SqlitePool,
    run_id: &str,
    status: &str,
    finding_count: i64,
    coverage_pct: f64,
) -> Result<()> {
    let completed_at = chrono::Utc::now().to_rfc3339();
    sqlx::query!(
        "UPDATE runs SET status = ?1, finding_count = ?2, coverage_pct = ?3, completed_at = ?4 WHERE id = ?5",
        status, finding_count, coverage_pct, completed_at, run_id
    )
    .execute(db)
    .await?;
    Ok(())
}

fn kind_str(kind: &FindingKind) -> &'static str {
    match kind {
        FindingKind::FuzzCrash => "fuzz_crash",
        FindingKind::InvariantViolation => "invariant_violation",
        FindingKind::CoverageEdge => "coverage_edge",
    }
}

fn severity_str(severity: &FindingSeverity) -> &'static str {
    match severity {
        FindingSeverity::Critical => "critical",
        FindingSeverity::High => "high",
        FindingSeverity::Medium => "medium",
        FindingSeverity::Low => "low",
        FindingSeverity::Info => "info",
    }
}
