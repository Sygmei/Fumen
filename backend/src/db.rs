use anyhow::{Context, Result};
use diesel_async::AsyncConnection;
use diesel_async::AsyncMigrationHarness;
use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::bb8::{Pool, RunError};
use diesel_migrations::{FileBasedMigrations, MigrationHarness};
use std::path::PathBuf;
use tracing::info;

pub(crate) type DbPool = Pool<AsyncPgConnection>;
pub(crate) type DbPoolError = RunError;

pub(crate) async fn open_database_pool(
    url: &str,
    max_connections: u32,
    role: &str,
) -> Result<DbPool> {
    let manager = AsyncDieselConnectionManager::<AsyncPgConnection>::new(url);

    Pool::builder()
        .max_size(max_connections)
        .build(manager)
        .await
        .with_context(|| format!("failed to open PostgreSQL connection pool for {role}"))
}

pub(crate) async fn run_migrations(database_url: &str) -> Result<()> {
    let migrations = FileBasedMigrations::from_path(migrations_dir())?;
    info!("starting database migrations");
    let connection = AsyncPgConnection::establish(database_url).await?;
    let mut harness = AsyncMigrationHarness::new(connection);
    harness
        .run_pending_migrations(migrations)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    info!("completed database migrations");
    Ok(())
}

fn migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}
