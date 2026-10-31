//! An open SQLite Recall store and the outcomes of its operations.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::records::models::{RecallValue, RowEnvelope};
use crate::schema::models::{ColumnType, Table};

/// A relational insert or lookup and its borrowed, typed parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct RecallStatement<'a> {
    pub sql: String,
    pub values: Vec<(&'a RecallValue, ColumnType)>,
}

/// One open store file. Only one thread uses a store.
#[derive(Debug)]
pub struct SqliteStore {
    pub(crate) connection: rusqlite::Connection,
    pub path: PathBuf,
}

/// Whether a store is opened for writing, creating it when missing, or only for reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenMode {
    ReadWrite,
    ReadOnly,
}

/// Which file a path named when it was observed: device and inode on Unix, else size and modification time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoreFileIdentity {
    pub(crate) volume: u64,
    pub(crate) file: u64,
}

/// One look at the file at a store path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoreObservation {
    pub identity: StoreFileIdentity,
    /// The file has content and does not start with the SQLite header.
    pub header_foreign: bool,
}

/// What moving a corrupt store aside did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuarantineOutcome {
    /// The observed file and its companions were renamed to this path.
    Moved(PathBuf),
    /// The path no longer holds the observed corrupt file, so the store there is used as it is.
    Kept,
    /// Another process held the quarantine lock past the wait; nothing was moved.
    LockBusy,
}

/// A row whose key was already stored with different content; the first row is kept.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ConflictEvidence {
    pub table: Table,
    pub record_key: String,
    pub existing_sha256: String,
    pub rejected_sha256: String,
}

/// What one insert batch did, per table.
#[derive(
    Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
pub struct InsertOutcome {
    /// Rows written or already present with equal content, by table.
    pub delivered: Vec<(Table, u64)>,
    pub conflicts: Vec<ConflictEvidence>,
    /// Tables whose insert failed, so their rows were not written, and the first failure.
    pub failed: Vec<Table>,
    pub failure: Option<String>,
}

/// What a maintenance run is allowed to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaintenanceRequest {
    pub now: SystemTime,
    pub retention_days: u32,
    pub owner: String,
    pub budget: Duration,
    pub envelope: RowEnvelope,
}

/// What a maintenance run did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MaintenanceOutcome {
    /// False when another process maintained the store within the interval.
    pub ran: bool,
    pub deleted_invocations: u64,
    pub deleted_artifacts: u64,
    /// False when the budget ran out with expired invocations left; the next run is overdue.
    pub complete: bool,
}

/// The rows of a read-only query.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueryResult {
    pub columns: Vec<String>,
    /// Each column's type as the store names it, where known.
    pub types: Vec<Option<String>>,
    pub rows: Vec<Vec<RecallValue>>,
    /// More rows existed beyond the limit.
    pub truncated: bool,
}

/// One column of a table or view, as the store describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescribedColumn {
    pub name: String,
    pub declared_type: String,
    pub nullable: bool,
}

/// One table or view of the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescribedRelation {
    pub name: String,
    /// `table` or `view`.
    pub kind: String,
    pub columns: Vec<DescribedColumn>,
}

/// One invocation's stored output and how complete it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputTranscript {
    pub invocation_id: String,
    /// The recorded bytes in order, with a note line where output past `max_output_bytes` was left out.
    pub data: Vec<u8>,
    pub status: Option<String>,
    pub missing_sequences: i64,
    pub reported_drops: i64,
    pub filtered: i64,
    pub flush_complete: bool,
    pub complete: bool,
    pub gap_reason: Option<String>,
}

/// One stored output row, as the transcript needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StoredChunk {
    pub(crate) stream: String,
    pub(crate) data: Vec<u8>,
    pub(crate) omitted_bytes: Option<i64>,
}
