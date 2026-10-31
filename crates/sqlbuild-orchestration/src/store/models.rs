//! Native database handles and bound values shared by control-plane operations.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use sqlbuild_recall::schema::models::ColumnType;
use sqlbuild_recall::store::models::ConflictEvidence;
use uuid::Uuid;

/// An installation's control store. Workers and locations never receive this handle.
#[derive(Clone, Debug)]
pub enum ControlStore {
    Sqlite(PathBuf),
    Postgres(sqlx_postgres::PgPool),
}

/// The first copy wins; duplicate events retain their original storage cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecallAppendOutcome {
    pub inserted: bool,
    pub storage_id: Option<i64>,
    pub conflict: Option<ConflictEvidence>,
}

/// A control-plane fact shares Recall's durable sequencing and conflict handling.
#[derive(Debug)]
pub(crate) struct ControlEvent<'a> {
    pub(crate) event_id: Uuid,
    pub(crate) invocation_id: String,
    pub(crate) event_type: &'a str,
    pub(crate) producer: &'static str,
    pub(crate) occurred_at: DateTime<Utc>,
    pub(crate) run_id: Option<Uuid>,
    pub(crate) payload: serde_json::Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransactionMode {
    Read,
    Write,
}

/// An owned transaction; abandoning it rolls back, including when an async task is cancelled.
#[derive(Debug)]
pub(crate) struct ControlTransaction {
    pub(crate) connection: Option<Connection>,
    pub(crate) now: DateTime<Utc>,
}

#[derive(Debug)]
pub(crate) enum Connection {
    Sqlite(rusqlite::Connection),
    Postgres(sqlx_core::transaction::Transaction<'static, sqlx_postgres::Postgres>),
}

/// Parameters keep their PostgreSQL types; SQLite uses their corresponding storage classes.
#[derive(Clone, Debug)]
pub(crate) enum Parameter {
    Null(ColumnType),
    Text(String),
    OptionalText(Option<String>),
    Bytes(Vec<u8>),
    Integer(i64),
    Real(f64),
    Json(serde_json::Value),
    Boolean(bool),
    Id(Uuid),
    OptionalId(Option<Uuid>),
    Time(DateTime<Utc>),
    OptionalTime(Option<DateTime<Utc>>),
}
