//! Native database handles and bound values shared by control-plane operations.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlbuild_recall::schema::models::ColumnType;
use sqlbuild_recall::store::models::ConflictEvidence;
use tokio::sync::broadcast;
use uuid::Uuid;

/// An installation's control store. Workers and locations never receive this handle.
#[derive(Clone, Debug)]
pub enum ControlStore {
    Sqlite(PathBuf, Arc<std::fs::File>),
    Postgres(sqlx_postgres::PgPool),
}

/// Only the offline opener can establish the exclusive-access capability.
#[derive(Debug)]
pub struct OfflineRecovery(pub(crate) ControlTransaction);

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
    pub(crate) changes: BTreeSet<Change>,
    pub(crate) notifications: Option<broadcast::Sender<ChangeMessage>>,
}

#[derive(Debug)]
pub(crate) enum Connection {
    Sqlite(rusqlite::Connection, Arc<std::fs::File>),
    Postgres(sqlx_core::transaction::Transaction<'static, sqlx_postgres::Postgres>),
}

/// Transaction-local invalidations, not another durable lifecycle or event log.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Change {
    Run(Uuid),
    Trigger(Uuid),
    Snapshot(Uuid, String),
    NodeStatus(Uuid, String),
    Credentials,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ChangeTopic {
    Runs,
    Run(Uuid),
    Ticks,
    Snapshot,
    NodeStatus,
}

/// Fixed-size routing payloads fit PostgreSQL NOTIFY even with long target labels.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ChangeMessage {
    Changed {
        location_id: Uuid,
        target_hash: String,
        topic: ChangeTopic,
    },
    CredentialsChanged,
    Resync,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChangedScope {
    pub(crate) location_id: Uuid,
    pub(crate) target: String,
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
