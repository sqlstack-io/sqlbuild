//! Store fixtures retain their isolated SQLite directory or PostgreSQL schema until cleanup.

use sqlbuild_orchestration::store::models::ControlStore;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Backend {
    Sqlite,
    Postgres,
}

#[derive(Debug)]
pub(crate) struct TestStore {
    pub(crate) store: ControlStore,
    pub(crate) directory: Option<tempfile::TempDir>,
    pub(crate) postgres: Option<(sqlx_postgres::PgPool, String)>,
}
