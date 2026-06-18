//! GET /coverage/:contract   — historical coverage snapshots for a contract

use axum::{
    Router,
    extract::{Path, State},
    routing::get,
    Json,
};

use crate::{error::ApiResult, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/coverage/:contract", get(get_coverage))
}

async fn get_coverage(
    State(state): State<AppState>,
    Path(contract): Path<String>,
) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let rows = sqlx::query!(
        "SELECT run_id, contract_name, coverage_pct, unique_edges, snapshot_at
         FROM coverage_snapshots WHERE contract_name = ?1 ORDER BY snapshot_at ASC",
        contract
    )
    .fetch_all(&state.db)
    .await?;

    let snapshots = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "run_id": r.run_id,
                "contract_name": r.contract_name,
                "coverage_pct": r.coverage_pct,
                "unique_edges": r.unique_edges,
                "snapshot_at": r.snapshot_at,
            })
        })
        .collect();

    Ok(Json(snapshots))
}
