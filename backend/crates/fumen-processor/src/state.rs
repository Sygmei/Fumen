use fumen_core::{config::AppConfig, db::DbPool, storage::Storage};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db_rw: DbPool,
    pub db_ro: DbPool,
    pub storage: Storage,
}
