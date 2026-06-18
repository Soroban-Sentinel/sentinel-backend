//! A `Run` tracks one full Sentinel execution (fuzz + verify) triggered by a CI event.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Passed,
    Failed,
    Errored,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: Uuid,
    /// Git repository URL.
    pub repo: String,
    /// Commit SHA that triggered this run.
    pub commit_sha: String,
    /// PR / MR number if triggered by a pull request.
    pub pr_number: Option<u64>,
    pub status: RunStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    /// Aggregate fuzz coverage percentage (0–100).
    pub coverage_pct: Option<f32>,
    /// Total number of findings in this run.
    pub finding_count: u32,
}

impl Run {
    pub fn new(repo: impl Into<String>, commit_sha: impl Into<String>, pr_number: Option<u64>) -> Self {
        Self {
            id: Uuid::new_v4(),
            repo: repo.into(),
            commit_sha: commit_sha.into(),
            pr_number,
            status: RunStatus::Queued,
            created_at: Utc::now(),
            completed_at: None,
            coverage_pct: None,
            finding_count: 0,
        }
    }
}
