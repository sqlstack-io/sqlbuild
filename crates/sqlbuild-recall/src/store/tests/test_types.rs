use crate::records::models::{RecallRow, RecallValue};
use crate::store::errors::StoreErrorKind;
use crate::store::models::{MaintenanceOutcome, StoreObservation};

pub(crate) struct ProjectionTestCase {
    pub(crate) description: &'static str,
    pub(crate) batches: Vec<Vec<RecallRow>>,
    pub(crate) sql: &'static str,
    pub(crate) expected_rows: Vec<Vec<RecallValue>>,
}

pub(crate) struct PayloadTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_payload: &'static str,
}

pub(crate) struct ResolveInvocationTestCase {
    pub(crate) description: &'static str,
    pub(crate) selector: &'static str,
    pub(crate) expected_invocation: Result<&'static str, StoreErrorKind>,
}

pub(crate) struct ChosenInvocationTestCase {
    pub(crate) description: &'static str,
    pub(crate) candidates: Vec<String>,
    pub(crate) expected_message: String,
}

pub(crate) struct QueryTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) limit: Option<usize>,
    pub(crate) expected_shape: Result<(Vec<String>, usize, bool), StoreErrorKind>,
}

pub(crate) struct TranscriptTestCase {
    pub(crate) description: &'static str,
    pub(crate) invocation_id: &'static str,
    pub(crate) expected_transcript: Result<(Vec<u8>, bool), StoreErrorKind>,
}

pub(crate) struct NewerSchemaTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_rows: Vec<Vec<RecallValue>>,
    pub(crate) expected_first_kind: &'static str,
    pub(crate) expected_data_nullable: Option<bool>,
}

pub(crate) struct LockedDirectoryTestCase {
    pub(crate) description: &'static str,
    pub(crate) relative_path: &'static str,
    pub(crate) expected_kind: Result<(), StoreErrorKind>,
}

pub(crate) struct MaintenanceTestCase {
    pub(crate) description: &'static str,
    pub(crate) batch: Vec<RecallRow>,
    pub(crate) runs: &'static [(i64, u64)],
    pub(crate) sql: &'static str,
    pub(crate) expected_outcomes: Vec<MaintenanceOutcome>,
    pub(crate) expected_rows: Vec<Vec<RecallValue>>,
}

/// What another process does between observing a corrupt store and moving it aside.
pub(crate) type QuarantineRace = fn(&std::path::Path, &StoreObservation, &std::fs::File);

pub(crate) struct QuarantineTestCase {
    pub(crate) description: &'static str,
    pub(crate) race: QuarantineRace,
    /// Whether the move was made, copies moved aside, and whether a SQLite store is at the path.
    pub(crate) expected_outcome: (bool, usize, bool),
}

pub(crate) struct TruncationNoteTestCase {
    pub(crate) description: &'static str,
    pub(crate) omitted: u64,
    pub(crate) kept: (u64, u64),
    pub(crate) expected_note: &'static str,
}
