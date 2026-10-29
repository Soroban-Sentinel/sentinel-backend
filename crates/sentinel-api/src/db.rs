use anyhow::Result;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use tracing::info;

pub async fn connect() -> Result<SqlitePool> {
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://sentinel.db".into());
    if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
        info!("Creating SQLite database");
        Sqlite::create_database(&db_url).await?;
    }
    let pool = SqlitePool::connect(&db_url).await?;
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

/// A process restart cannot resume a detached analysis job safely.
pub async fn recover_interrupted(pool: &SqlitePool) -> Result<()> {
    sqlx::query("UPDATE runs SET status = 'errored', completed_at = ?1, error_message = 'Server restarted before analysis completed' WHERE status IN ('queued', 'running')")
        .bind(chrono::Utc::now().to_rfc3339()).execute(pool).await?;
    Ok(())
}
