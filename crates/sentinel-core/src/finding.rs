//! A `Finding` is a single discovered invariant violation or crash.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    /// Fuzzer found a panic / trap.
    FuzzCrash,
    /// SMT solver proved an invariant is violated.
    InvariantViolation,
    /// Fuzzer discovered a new coverage edge (informational).
    CoverageEdge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: Uuid,
    pub run_id: Uuid,
    pub contract_name: String,
    pub kind: FindingKind,
    pub severity: FindingSeverity,
    /// Human-readable description of the finding.
    pub description: String,
    /// The reproducer input (hex-encoded bytes for fuzz crashes, SMT model for violations).
    pub reproducer: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Finding {
    pub fn new(
        run_id: Uuid,
        contract_name: impl Into<String>,
        kind: FindingKind,
        severity: FindingSeverity,
        description: impl Into<String>,
        reproducer: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            run_id,
            contract_name: contract_name.into(),
            kind,
            severity,
            description: description.into(),
            reproducer,
            created_at: Utc::now(),
        }
    }
}
