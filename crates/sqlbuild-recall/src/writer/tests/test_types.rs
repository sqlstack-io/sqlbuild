use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::records::models::{OutputChunk, OutputStream, RecallRow, RecallValue, RowEnvelope};
use crate::schema::models::Table;
use crate::settings::models::AuditEvidence;
use crate::store::errors::{StoreError, StoreErrorKind};
use crate::store::models::InsertOutcome;
use crate::store::types::{RecallStore, RecallStoreOpener};
use crate::writer::models::{
    DeliveryStream, Priority, ReplacedEvents, ReplacedOutput, SetupOutcome,
};
use crate::writer::types::{OutputReplacement, PendingLifecycleEvent};

/// What one recorded session left in the store.
#[derive(Debug, PartialEq)]
pub(crate) struct RecordedSession {
    pub(crate) setup: Option<SetupOutcome>,
    pub(crate) lost: (u64, u64),
    pub(crate) output_fully_delivered: bool,
    pub(crate) transcript: (Vec<u8>, bool),
    pub(crate) stdout: Vec<u8>,
    pub(crate) facts: Vec<Vec<RecallValue>>,
}

pub(crate) struct DisplacementTestCase {
    pub(crate) description: &'static str,
    pub(crate) capacity: usize,
    pub(crate) pushes: &'static [(DeliveryStream, Priority)],
    pub(crate) expected_dropped: Vec<(DeliveryStream, u64)>,
    pub(crate) expected_taken: Vec<DeliveryStream>,
}

pub(crate) struct ChunkingTestCase {
    pub(crate) description: &'static str,
    pub(crate) writes: Vec<(OutputStream, Vec<u8>)>,
    pub(crate) expected_chunks: Vec<[Option<RecallValue>; 3]>,
}

pub(crate) struct SessionTestCase {
    pub(crate) description: &'static str,
    pub(crate) output: Vec<(OutputStream, Vec<u8>)>,
    pub(crate) expected_session: RecordedSession,
}

pub(crate) struct UnwritableTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_permission_and_degraded: (bool, bool),
}

pub(crate) struct HeldLockTestCase {
    pub(crate) description: &'static str,
    pub(crate) shutdown_ms: u64,
    pub(crate) expected_degraded: bool,
}

pub(crate) struct CorruptStoreTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_notices: usize,
    pub(crate) expected_moved: bool,
    pub(crate) expected_rows: Vec<Vec<RecallValue>>,
}

pub(crate) struct ArtifactSizeTestCase {
    pub(crate) description: &'static str,
    pub(crate) length: usize,
    pub(crate) times: usize,
    pub(crate) expected_counts: (u64, u64, u64, Vec<Vec<RecallValue>>),
}

pub(crate) struct EventBuildTestCase {
    pub(crate) description: &'static str,
    pub(crate) events: Vec<&'static str>,
    pub(crate) expected_counts: (u64, u64, u64),
}

pub(crate) struct OutputDropTestCase {
    pub(crate) description: &'static str,
    pub(crate) capacity: usize,
    pub(crate) chunks: usize,
    pub(crate) expected_drops: (u64, Vec<Option<RecallValue>>),
}

pub(crate) struct CaptureLimitTestCase {
    pub(crate) description: &'static str,
    pub(crate) limit: Option<u64>,
    pub(crate) writes: Vec<(OutputStream, &'static [u8])>,
    pub(crate) expected_rows: Vec<[Option<RecallValue>; 6]>,
}

pub(crate) struct StoredWhileRunningTestCase {
    pub(crate) description: &'static str,
    pub(crate) heartbeat: std::time::Duration,
    pub(crate) wait: std::time::Duration,
    pub(crate) expected_rows: Vec<Vec<RecallValue>>,
}

pub(crate) struct ReplacementTestCase {
    pub(crate) description: &'static str,
    pub(crate) output: &'static [(OutputStream, &'static [u8])],
    /// How long discovery takes after the output is written, before the replacement is set.
    pub(crate) discovery: Duration,
    pub(crate) expected_transcript: &'static [u8],
    pub(crate) expected_accounting: Vec<Vec<RecallValue>>,
}

/// Publishes `keep…` chunks upper-cased, filters `drop…` chunks and fails on anything else.
#[derive(Debug)]
pub(crate) struct PrefixReplacement;

impl OutputReplacement for PrefixReplacement {
    fn replace(&self, chunk: &OutputChunk) -> ReplacedOutput {
        if chunk.data.starts_with(b"keep") {
            ReplacedOutput::Published(chunk.data.to_ascii_uppercase())
        } else if chunk.data.starts_with(b"drop") {
            ReplacedOutput::Filtered
        } else {
            ReplacedOutput::Failed
        }
    }
}

pub(crate) struct PartialInsertTestCase {
    pub(crate) description: &'static str,
    pub(crate) failing: Table,
    /// `(accepted, delivered, failed)` of the events and output streams.
    pub(crate) expected_counts: [(u64, u64, u64); 2],
}

/// A hosted store whose insert into `failing` fails while the batch's other tables are written.
#[derive(Debug)]
pub(crate) struct PartialStore {
    pub(crate) failing: Table,
}

impl RecallStore for PartialStore {
    fn insert(&self, rows: &[RecallRow], _give_up: Instant) -> Result<InsertOutcome, StoreError> {
        let failing: bool = rows.iter().any(|row| row.table == self.failing);
        Ok(InsertOutcome {
            failed: if failing {
                vec![self.failing]
            } else {
                Vec::new()
            },
            failure: failing.then(|| "the insert was refused".to_owned()),
            ..InsertOutcome::default()
        })
    }
}

impl RecallStoreOpener for PartialStore {
    fn open(
        &self,
        _envelope: &RowEnvelope,
        _give_up: Instant,
    ) -> Result<Box<dyn RecallStore>, StoreError> {
        Ok(Box::new(Self {
            failing: self.failing,
        }))
    }

    fn batch_rows(&self) -> usize {
        1_000
    }

    fn batch_window(&self) -> Duration {
        Duration::from_millis(10)
    }
}

pub(crate) struct EarlyBatchTestCase {
    pub(crate) description: &'static str,
    pub(crate) written_bytes: usize,
    pub(crate) wait: Duration,
    pub(crate) expected_minimum_sent_bytes: usize,
    pub(crate) expected_dropped: u64,
}

/// A hosted store with a batch window far longer than any test, counting the output bytes it receives.
#[derive(Clone, Debug, Default)]
pub(crate) struct OutputBytesStore {
    pub(crate) received: Arc<Mutex<usize>>,
}

impl RecallStore for OutputBytesStore {
    fn insert(&self, rows: &[RecallRow], _give_up: Instant) -> Result<InsertOutcome, StoreError> {
        let bytes: usize = rows
            .iter()
            .filter(|row| row.table == Table::Output)
            .map(|row| match row.value("data") {
                Some(RecallValue::Blob(data)) => data.len(),
                _ => 0,
            })
            .sum();
        *self
            .received
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) += bytes;
        Ok(InsertOutcome::default())
    }
}

impl RecallStoreOpener for OutputBytesStore {
    fn open(
        &self,
        _envelope: &RowEnvelope,
        _give_up: Instant,
    ) -> Result<Box<dyn RecallStore>, StoreError> {
        Ok(Box::new(self.clone()))
    }

    fn batch_rows(&self) -> usize {
        1_000_000
    }

    fn batch_window(&self) -> Duration {
        Duration::from_secs(600)
    }
}

pub(crate) struct DiscoveryTestCase {
    pub(crate) description: &'static str,
    pub(crate) wait: Duration,
    pub(crate) expected_rows: Vec<Vec<RecallValue>>,
}

pub(crate) struct PendingEventTestCase {
    pub(crate) description: &'static str,
    /// How many copies of the event each replacement publishes; `None` when it fails.
    pub(crate) copies: Vec<Option<usize>>,
    /// `(accepted, delivered, filtered, failed)` of the events stream.
    pub(crate) expected_counts: (u64, u64, u64, u64),
}

/// A lifecycle event whose replacement publishes `copies` distinct copies of it, or fails.
#[derive(Debug)]
pub(crate) struct CopyingReplacement {
    pub(crate) index: usize,
    pub(crate) copies: Option<usize>,
}

impl PendingLifecycleEvent for CopyingReplacement {
    fn replace(self: Box<Self>) -> ReplacedEvents {
        let Some(copies) = self.copies else {
            return ReplacedEvents::Failed;
        };
        ReplacedEvents::Published(
            (0..copies)
                .map(|copy| {
                    serde_json::from_str(&format!(
                        r#"{{"event_id":"e{}-{copy}","event_type":"run_started","schema_version":2,
                        "producer":"sqlbuild","producer_version":"1",
                        "occurred_at":"2026-10-19T00:00:01.000000Z","invocation_id":"pending",
                        "payload":{{}}}}"#,
                        self.index
                    ))
                    .expect("the fixture is a JSON object")
                })
                .collect(),
        )
    }
}

pub(crate) struct CoreReleaseStoreTestCase {
    pub(crate) description: &'static str,
    pub(crate) write: fn(&std::path::Path),
    pub(crate) expected_rows: Vec<Vec<RecallValue>>,
}

pub(crate) struct ReplacementOrderTestCase {
    pub(crate) description: &'static str,
    pub(crate) priorities: Vec<Priority>,
    pub(crate) expected_order: Vec<usize>,
}

/// A lifecycle event that records when its replacement runs and keeps nothing.
#[derive(Debug)]
pub(crate) struct RecordingReplacement {
    pub(crate) index: usize,
    pub(crate) seen: std::sync::Arc<std::sync::Mutex<Vec<usize>>>,
}

impl PendingLifecycleEvent for RecordingReplacement {
    fn replace(self: Box<Self>) -> ReplacedEvents {
        self.seen
            .lock()
            .expect("the record is not poisoned")
            .push(self.index);
        ReplacedEvents::Published(Vec::new())
    }
}

pub(crate) struct CreationNoticeTestCase {
    pub(crate) description: &'static str,
    /// The failure of each open after the first creates the database; `None` opens the store.
    pub(crate) failures: Vec<Option<StoreErrorKind>>,
    pub(crate) expected_information: Vec<String>,
    pub(crate) expected_permission_denied: bool,
}

/// A hosted store whose first open creates its database, then fails or opens as `failures` say.
#[derive(Debug, Default)]
pub(crate) struct CreatingStore {
    pub(crate) failures: Vec<Option<StoreErrorKind>>,
    pub(crate) attempts: Mutex<usize>,
    pub(crate) information: Mutex<Vec<String>>,
}

impl RecallStoreOpener for CreatingStore {
    fn open(
        &self,
        _envelope: &RowEnvelope,
        _give_up: Instant,
    ) -> Result<Box<dyn RecallStore>, StoreError> {
        let mut attempts = self
            .attempts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let attempt: usize = *attempts;
        *attempts += 1;
        if attempt == 0 {
            self.information
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push("created database `history`".to_owned());
        }
        match self.failures.get(attempt).copied().flatten() {
            Some(kind) => Err(StoreError {
                kind,
                message: "the store could not be set up".to_owned(),
            }),
            None => Ok(Box::new(OutputBytesStore::default())),
        }
    }

    fn take_information(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .information
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    fn batch_rows(&self) -> usize {
        1_000
    }

    fn batch_window(&self) -> Duration {
        Duration::from_millis(10)
    }
}

pub(crate) struct AuditEvidenceTestCase {
    pub(crate) description: &'static str,
    pub(crate) audit_evidence: AuditEvidence,
    pub(crate) expected_rows: Vec<Vec<RecallValue>>,
}

pub(crate) struct FailedStatementTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_rows: Vec<Vec<RecallValue>>,
}

pub(crate) struct SlowFinalRowsTestCase {
    pub(crate) description: &'static str,
    pub(crate) shutdown: Duration,
    /// How long the store takes to accept the final delivery rows.
    pub(crate) final_rows_delay: Duration,
    pub(crate) expected_flush_complete: bool,
}

/// A hosted store whose final delivery rows take `delay`, giving up at the deadline like HTTP.
#[derive(Clone, Debug)]
pub(crate) struct SlowFinalRowsStore {
    pub(crate) delay: Duration,
}

impl RecallStore for SlowFinalRowsStore {
    fn insert(&self, rows: &[RecallRow], give_up: Instant) -> Result<InsertOutcome, StoreError> {
        if !rows.iter().any(|row| row.table == Table::Delivery) {
            return Ok(InsertOutcome::default());
        }
        let answered: Instant = Instant::now() + self.delay;
        std::thread::sleep(
            answered
                .min(give_up)
                .saturating_duration_since(Instant::now()),
        );
        if answered > give_up {
            return Err(StoreError::new(
                StoreErrorKind::Busy,
                "the deadline expired",
            ));
        }
        Ok(InsertOutcome::default())
    }
}

impl RecallStoreOpener for SlowFinalRowsStore {
    fn open(
        &self,
        _envelope: &RowEnvelope,
        _give_up: Instant,
    ) -> Result<Box<dyn RecallStore>, StoreError> {
        Ok(Box::new(self.clone()))
    }

    fn batch_rows(&self) -> usize {
        1_000
    }

    fn batch_window(&self) -> Duration {
        Duration::from_millis(10)
    }
}
