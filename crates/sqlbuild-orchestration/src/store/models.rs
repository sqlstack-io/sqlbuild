//! Native database handles and bound values shared by control-plane operations.

use std::collections::BTreeSet;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlbuild_recall::schema::models::ColumnType;
use sqlbuild_recall::store::models::ConflictEvidence;
use tokio::sync::broadcast;
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

/// A row whose location and target decide whether a person's target-scoped read may name it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum TargetRow {
    Run(Uuid),
    Trigger(Uuid),
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
    Sqlite(rusqlite::Connection),
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

impl Parameter {
    /// Numbered placeholders for the parameters at these positions, such as an IN list.
    pub(crate) fn placeholders(positions: std::ops::RangeInclusive<usize>) -> String {
        positions
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>()
            .join(",")
    }
}
