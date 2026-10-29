//! Soroban Sentinel API server entry point.

mod auth;
mod db;
mod error;
mod routes;
mod state;
mod worker;

use anyhow::Result;
use axum::Router;
use std::{net::SocketAddr, path::PathBuf};
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("sentinel_api=info".parse()?))
        .init();

    let api_token = std::env::var("SENTINEL_API_TOKEN")
        .map_err(|_| anyhow::anyhow!("SENTINEL_API_TOKEN is required"))?;
    anyhow::ensure!(
        api_token.len() >= 32 && api_token.bytes().all(|b| b.is_ascii_graphic()),
        "SENTINEL_API_TOKEN must be at least 32 bytes"
    );
    let pool = db::connect().await?;
    db::migrate(&pool).await?;
    db::recover_interrupted(&pool).await?;

    let workspace_root = std::env::var("SENTINEL_WORKSPACE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/sentinel-workspace"));

    info!("Workspace root: {}", workspace_root.display());

    let state = state::AppState::new(pool, workspace_root, api_token);

    let app = Router::new()
        .merge(routes::runs::router())
        .merge(routes::findings::router())
        .merge(routes::coverage::router())
        .layer(TraceLayer::new_for_http())
        .layer(axum::extract::DefaultBodyLimit::max(65536))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_token,
        ))
        .with_state(state);

    let addr: SocketAddr = std::env::var("SENTINEL_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;
    info!("Sentinel API listening on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;

    Ok(())
}
