use anyhow::Result;
use sqlx::{SqlitePool, migrate::MigrateDatabase, Sqlite};
use tracing::info;

const DB_URL: &str = "sqlite://sentinel.db";

pub async fn connect() -> Result<SqlitePool> {
    if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
        info!("Creating database at {}", DB_URL);
        Sqlite::create_database(DB_URL).await?;
    }
    let pool = SqlitePool::connect(DB_URL).await?;
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
