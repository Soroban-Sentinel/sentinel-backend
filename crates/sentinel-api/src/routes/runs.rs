//! POST /runs       — trigger a new Sentinel run
//! GET  /runs       — list all runs
//! GET  /runs/:id   — get a single run

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use sentinel_core::config::SentinelConfig;
use uuid::Uuid;

use crate::{error::ApiResult, state::AppState, worker};

#[derive(Debug, Deserialize)]
pub struct CreateRunRequest {
    pub repo: String,
    pub commit_sha: String,
    pub pr_number: Option<u64>,
    /// Base64-encoded `.sentinel.toml` content.
    pub config_b64: String,
}

#[derive(Debug, Serialize)]
pub struct RunResponse {
    pub id: String,
    pub status: String,
    pub repo: String,
    pub commit_sha: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/runs", post(create_run))
        .route("/runs", get(list_runs))
        .route("/runs/:id", get(get_run))
}

async fn create_run(
    State(state): State<AppState>,
    Json(req): Json<CreateRunRequest>,
) -> ApiResult<(StatusCode, Json<RunResponse>)> {
    // Decode and parse the config early so we reject bad input before touching the DB.
    let toml_bytes = BASE64.decode(&req.config_b64)?;
    let toml_str = String::from_utf8(toml_bytes)?;
    let config = SentinelConfig::from_toml(&toml_str)
        .map_err(|e| anyhow::anyhow!("invalid sentinel config: {}", e))?;

    let id = Uuid::new_v4().to_string();

    let pr_number = req.pr_number.map(|n| n as i64);
    sqlx::query!(
        "INSERT INTO runs (id, repo, commit_sha, pr_number, status, created_at)
         VALUES (?1, ?2, ?3, ?4, 'queued', datetime('now'))",
        id,
        req.repo,
        req.commit_sha,
        pr_number,
    )
    .execute(&state.db)
    .await?;

    // Spawn background job — detached; errors are logged inside execute_run.
    let run_id = id.clone();
    tokio::spawn(worker::execute_run(
        state.db.clone(),
        run_id,
        config,
        state.workspace_root.clone(),
    ));

    Ok((
        StatusCode::CREATED,
        Json(RunResponse {
            id,
            status: "queued".into(),
            repo: req.repo,
            commit_sha: req.commit_sha,
        }),
    ))
}

async fn list_runs(State(state): State<AppState>) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let rows = sqlx::query!(
        "SELECT id, repo, commit_sha, status, created_at, coverage_pct, finding_count FROM runs ORDER BY created_at DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await?;

    let runs: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "repo": r.repo,
                "commit_sha": r.commit_sha,
                "status": r.status,
                "created_at": r.created_at,
                "coverage_pct": r.coverage_pct,
                "finding_count": r.finding_count,
            })
        })
        .collect();

    Ok(Json(runs))
}

async fn get_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let r = sqlx::query!(
        "SELECT id, repo, commit_sha, status, created_at, coverage_pct, finding_count FROM runs WHERE id = ?1",
        id
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "id": r.id,
        "repo": r.repo,
        "commit_sha": r.commit_sha,
        "status": r.status,
        "created_at": r.created_at,
        "coverage_pct": r.coverage_pct,
        "finding_count": r.finding_count,
    })))
}
