//! Run dynamic checks against an isolated checkout of the submitted Git commit.
use anyhow::Result;
use sentinel_core::{
    config::SentinelConfig,
    finding::{FindingKind, FindingSeverity},
};
use sentinel_fuzzer::{analysis::analyze, workspace::snapshot};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tracing::error;
use uuid::Uuid;

pub async fn execute_run(
    db: SqlitePool,
    run_id: String,
    config: SentinelConfig,
    workspace_root: PathBuf,
) {
    let result = run_pipeline(&db, &run_id, &config, &workspace_root).await;
    let error_message = result.as_ref().err().map(|error| format!("{error:#}"));
    if let Some(message) = &error_message {
        error!(run_id = %run_id, error = %message, "Analysis failed");
    }
    let status = match result {
        Ok(0) => "passed",
        Ok(_) => "failed",
        Err(_) => "errored",
    };
    if let Err(error) = sqlx::query("UPDATE runs SET status = ?1, completed_at = ?2, error_message = ?3, finding_count = (SELECT COUNT(*) FROM findings WHERE run_id = ?4), coverage_pct = NULL WHERE id = ?4")
        .bind(status).bind(chrono::Utc::now().to_rfc3339()).bind(error_message).bind(&run_id).execute(&db).await {
        error!(%run_id, %error, "Could not finalize analysis");
    }
}

async fn run_pipeline(
    db: &SqlitePool,
    run_id: &str,
    config: &SentinelConfig,
    workspace_root: &Path,
) -> Result<usize> {
    config.validate()?;
    sqlx::query("UPDATE runs SET status = 'running' WHERE id = ?")
        .bind(run_id)
        .execute(db)
        .await?;
    let (repository, commit): (String, String) =
        sqlx::query_as("SELECT repo, commit_sha FROM runs WHERE id = ?")
            .bind(run_id)
            .fetch_one(db)
            .await?;
    let checkout = snapshot(workspace_root, &repository, &commit).await?;
    let mut results = Vec::new();
    let mut total_findings = 0;
    for contract in &config.contracts {
        let result = analyze(&checkout.root(), Uuid::parse_str(run_id)?, contract).await?;
        total_findings += result.findings.len();
        let mut transaction = db.begin().await?;
        for finding in &result.findings {
            sqlx::query("INSERT INTO findings (id, run_id, contract_name, kind, severity, description, reproducer, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(finding.id.to_string()).bind(run_id).bind(&finding.contract_name).bind(kind_str(&finding.kind)).bind(severity_str(&finding.severity))
                .bind(&finding.description).bind(&finding.reproducer).bind(finding.created_at.to_rfc3339()).execute(&mut *transaction).await?;
        }
        // Historical schema requires a number here; zero means unknown, never a measured percentage.
        if contract.analysis == sentinel_core::config::AnalysisMode::Fuzz {
            sqlx::query("INSERT INTO coverage_snapshots (run_id, contract_name, coverage_pct, unique_edges, snapshot_at) VALUES (?, ?, 0, ?, ?)")
                .bind(run_id).bind(&contract.name).bind(i64::try_from(result.unique_edges)?).bind(chrono::Utc::now().to_rfc3339()).execute(&mut *transaction).await?;
        }
        results.push(result);
        let report = serde_json::json!({"source_commit": checkout.commit, "repository": checkout.repository, "scope": "dynamic analysis; not a formal proof", "results": results});
        sqlx::query("UPDATE runs SET analysis_report = ? WHERE id = ?")
            .bind(serde_json::to_string(&report)?)
            .bind(run_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
    }
    Ok(total_findings)
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
    async fn unavailable_source_cannot_pass_and_records_error() {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::db::migrate(&db).await.unwrap();
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO runs (id, repo, commit_sha, created_at) VALUES (?, 'test/token', ?, datetime('now'))").bind(&id).bind("a".repeat(40)).execute(&db).await.unwrap();
        let config = SentinelConfig::from_toml(
            r#"version = "1"
[[contracts]]
name = "token"
path = "token"
invariants = ["balance_conservation"]
analysis = "soroban_tests"
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
        let (status, completed, error): (String, Option<String>, Option<String>) =
            sqlx::query_as("SELECT status, completed_at, error_message FROM runs WHERE id = ?")
                .bind(id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(status, "errored");
        assert!(completed.is_some());
        assert!(error.is_some());
    }
    #[tokio::test]
    async fn recovers_jobs_interrupted_by_restart() {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::db::migrate(&db).await.unwrap();
        for status in ["queued", "running", "passed"] {
            sqlx::query("INSERT INTO runs (id, repo, commit_sha, status, created_at) VALUES (?, 'test/token', 'sha', ?, datetime('now'))").bind(status).bind(status).execute(&db).await.unwrap();
        }
        crate::db::recover_interrupted(&db).await.unwrap();
        let statuses: Vec<(String, String)> =
            sqlx::query_as("SELECT id, status FROM runs ORDER BY id")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(
            statuses,
            vec![
                ("passed".into(), "passed".into()),
                ("queued".into(), "errored".into()),
                ("running".into(), "errored".into())
            ]
        );
    }
}
