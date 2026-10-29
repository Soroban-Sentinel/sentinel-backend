//! Background job processor — executes a queued Sentinel run end-to-end.
//!
//! Requires a reviewed fuzz target, persists observed crashes and edge counts,
//! and treats unavailable analysis as an error rather than a successful check.

use anyhow::Result;
use sentinel_core::{
    config::SentinelConfig,
    finding::{Finding, FindingKind, FindingSeverity},
};
use sentinel_fuzzer::runner::FuzzRunner;
use sentinel_verifier::engine::VerificationEngine;
use sqlx::SqlitePool;
use std::path::PathBuf;
use tracing::{error, info};
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
            let status = if finding_count > 0 {
                "failed"
            } else {
                "passed"
            };
            if let Err(e) = finish_run(&db, &run_id, status, finding_count, coverage_pct).await {
                error!(run_id = %run_id, error = %e, "Failed to finalize run record");
            } else {
                info!(run_id = %run_id, status, finding_count, coverage_pct, "Run complete");
            }
        }
        Err(e) => {
            error!(run_id = %run_id, error = %e, "Run pipeline errored");
            let completed_at = chrono::Utc::now().to_rfc3339();
            let _ = sqlx::query("UPDATE runs SET status = 'errored', completed_at = ?1, finding_count = (SELECT COUNT(*) FROM findings WHERE run_id = ?2) WHERE id = ?2")
                .bind(completed_at).bind(&run_id).execute(&db).await;
        }
    }
}

async fn run_pipeline(
    db: &SqlitePool,
    run_id: &str,
    config: &SentinelConfig,
    workspace_root: &PathBuf,
) -> Result<(i64, f64)> {
    config.validate()?;
    let run_uuid = Uuid::parse_str(run_id)?;
    let mut total_findings: i64 = 0;
    let mut total_coverage: f64 = 0.0;

    for contract in &config.contracts {
        info!(run_id = %run_id, contract = %contract.name, "Processing contract");

        // Require a reviewed, contract-specific harness rather than a placeholder.
        let target = contract.name.replace('-', "_");
        anyhow::ensure!(
            workspace_root.join("fuzz/Cargo.toml").is_file(),
            "missing fuzz/Cargo.toml"
        );
        anyhow::ensure!(
            workspace_root
                .join("fuzz/fuzz_targets")
                .join(format!("{target}.rs"))
                .is_file(),
            "missing fuzz target: {target}"
        );

        // ── 2. Fuzzing ─────────────────────────────────────────────────────
        let fuzz_findings = {
            let fuzzer = FuzzRunner::new(workspace_root);
            match fuzzer.run(run_uuid, contract).await {
                Ok(result) => {
                    total_coverage += result.coverage_pct as f64;
                    let cov = result.coverage_pct as f64;
                    let snap_at = chrono::Utc::now().to_rfc3339();
                    let edges = i64::try_from(result.unique_edges)?;
                    sqlx::query!(
                        "INSERT INTO coverage_snapshots (run_id, contract_name, coverage_pct, unique_edges, snapshot_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        run_id, contract.name, cov, edges, snap_at
                    )
                    .execute(db)
                    .await?;
                    result.findings
                }
                Err(e) => return Err(e),
            }
        };

        // ── 4. Persist findings ────────────────────────────────────────────
        let all_findings: Vec<Finding> = fuzz_findings;
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
        // Persist observed fuzz evidence even when formal analysis is unavailable.
        VerificationEngine::verify(run_uuid, contract)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn missing_harness_never_passes_and_records_completion() {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::db::migrate(&db).await.unwrap();
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO runs (id, repo, commit_sha, created_at) VALUES (?, 'test/token', 'abc', datetime('now'))").bind(&id).execute(&db).await.unwrap();
        let config = SentinelConfig::from_toml(
            r#"version = "1"
[[contracts]]
name = "token"
path = "contracts/token"
invariants = ["balance_conservation"]
"#,
        )
        .unwrap();
        execute_run(
            db.clone(),
            id.clone(),
            config,
            PathBuf::from("/nonexistent-sentinel-workspace"),
        )
        .await;
        let (status, completed): (String, Option<String>) =
            sqlx::query_as("SELECT status, completed_at FROM runs WHERE id = ?")
                .bind(id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(status, "errored");
        assert!(completed.is_some());
    }
}
