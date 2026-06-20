use sqlx::SqlitePool;
use std::path::PathBuf;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    /// Root directory of the contract workspace used for harness writing and fuzzing.
    /// Defaults to $SENTINEL_WORKSPACE_ROOT or /tmp/sentinel-workspace.
    pub workspace_root: PathBuf,
}

impl AppState {
    pub fn new(db: SqlitePool, workspace_root: PathBuf) -> Self {
        Self { db, workspace_root }
    }
}
