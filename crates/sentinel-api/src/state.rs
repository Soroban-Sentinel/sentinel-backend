use sqlx::SqlitePool;
use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub api_token: Arc<str>,
    pub run_slots: Arc<tokio::sync::Semaphore>,
    /// Root of the trusted source repository; jobs execute in disposable Git clones.
    /// Defaults to $SENTINEL_WORKSPACE_ROOT or /tmp/sentinel-workspace.
    pub workspace_root: PathBuf,
}

impl AppState {
    pub fn new(db: SqlitePool, workspace_root: PathBuf, api_token: String) -> Self {
        Self {
            db,
            workspace_root,
            api_token: api_token.into(),
            run_slots: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}
