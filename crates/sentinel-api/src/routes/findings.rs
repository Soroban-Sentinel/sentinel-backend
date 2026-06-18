//! GET /runs/:run_id/findings   — list findings for a run
//! GET /findings/:id            — get a single finding

use axum::{
    Router,
    extract::{Path, State},
    routing::get,
    Json,
};

use crate::{error::ApiResult, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/runs/:run_id/findings", get(list_findings))
        .route("/findings/:id", get(get_finding))
}

async fn list_findings(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let rows = sqlx::query!(
        "SELECT id, run_id, contract_name, kind, severity, description, reproducer, created_at
         FROM findings WHERE run_id = ?1 ORDER BY created_at DESC",
        run_id
    )
    .fetch_all(&state.db)
    .await?;

    let findings = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "run_id": r.run_id,
                "contract_name": r.contract_name,
                "kind": r.kind,
                "severity": r.severity,
                "description": r.description,
                "reproducer": r.reproducer,
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(Json(findings))
}

async fn get_finding(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let r = sqlx::query!(
        "SELECT id, run_id, contract_name, kind, severity, description, reproducer, created_at
         FROM findings WHERE id = ?1",
        id
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "id": r.id,
        "run_id": r.run_id,
        "contract_name": r.contract_name,
        "kind": r.kind,
        "severity": r.severity,
        "description": r.description,
        "reproducer": r.reproducer,
        "created_at": r.created_at,
    })))
}
