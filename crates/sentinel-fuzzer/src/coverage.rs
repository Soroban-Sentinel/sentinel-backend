//! Aggregates and persists coverage data across fuzz runs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageSnapshot {
    pub run_id: String,
    pub contract_name: String,
    /// Edge coverage 0–100.
    pub coverage_pct: f32,
    /// Total unique edges discovered.
    pub unique_edges: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl CoverageSnapshot {
    pub fn new(run_id: impl Into<String>, contract_name: impl Into<String>, coverage_pct: f32, unique_edges: u64) -> Self {
        Self {
            run_id: run_id.into(),
            contract_name: contract_name.into(),
            coverage_pct,
            unique_edges,
            timestamp: chrono::Utc::now(),
        }
    }
}
