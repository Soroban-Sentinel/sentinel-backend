//! Soroban Sentinel API server entry point.

mod db;
mod error;
mod routes;
mod state;

use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialise tracing
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("sentinel_api=info".parse()?))
        .init();

    // Database
    let pool = db::connect().await?;
    db::migrate(&pool).await?;

    // App state
    let state = state::AppState::new(pool);

    // Router
    let app = Router::new()
        .merge(routes::runs::router())
        .merge(routes::findings::router())
        .merge(routes::coverage::router())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("Sentinel API listening on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;

    Ok(())
}
