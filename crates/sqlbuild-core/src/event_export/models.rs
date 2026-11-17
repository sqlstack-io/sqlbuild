//! Export severities, policies, sink filters, attempt counters and the bounded priority queue.

use std::collections::BTreeSet;

/// `EventExportSeverity`, ordered from least to most severe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExportSeverity {
    Debug,
    Info,
    Warning,
    Error,
}

/// Catalogued export dimensions of one lifecycle event type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExportPolicy {
    pub kind: String,
    pub severity: ExportSeverity,
    pub priority: u8,
}

/// The event kinds and minimum severity one bound sink accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExporterFilter {
    pub event_kinds: BTreeSet<String>,
    pub min_severity: ExportSeverity,
}

/// Attempt counters for one exporter or their aggregate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ExporterCounts {
    pub accepted: u64,
    pub filtered: u64,
    pub delivered: u64,
    pub failed: u64,
    pub dropped: u64,
}

/// One queued event and the exact exporters still owed an attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedEvent<T> {
    pub sequence: u64,
    pub item: T,
    pub priority: u8,
    /// Whether the event ends the invocation and so waits behind every other queued event.
    pub invocation_terminal: bool,
    /// Whether the event reports a run or invocation outcome, which the queue always admits.
    pub outcome: bool,
    /// The invocation the event belongs to, when it names one.
    pub invocation_id: Option<String>,
    /// Whether the event starts its invocation and so goes before that invocation's other events.
    pub invocation_start: bool,
    pub eligible_exporters: Vec<usize>,
}

/// Bounded queue: highest priority first, FIFO within a priority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventQueue<T> {
    pub capacity: usize,
    pub items: Vec<QueuedEvent<T>>,
}

/// The queue after an insert, whether the event was queued, and the event it displaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueOutcome<T> {
    pub queue: EventQueue<T>,
    pub inserted: bool,
    pub displaced: Option<QueuedEvent<T>>,
    /// The incoming event, handed back when it was not queued.
    pub rejected: Option<QueuedEvent<T>>,
}

/// Final or point-in-time delivery accounting of every exporter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExportSnapshot {
    pub aggregate: ExporterCounts,
    pub per_exporter: Vec<(String, ExporterCounts)>,
    pub queue_depth: usize,
    pub queue_capacity: usize,
    pub flush_complete: bool,
}

/// What a snapshot is taken of: exporters, the queue and whether delivery has fully stopped.
#[derive(Clone, Copy, Debug)]
pub struct SnapshotInputs<'a> {
    pub exporters: &'a [(String, ExporterCounts)],
    pub queue_depth: usize,
    pub queue_capacity: usize,
    /// A final snapshot taken after the dispatcher and every invocation finished.
    pub drained: bool,
}

impl ExportSeverity {
    /// The severity named `value`, as `EventExportSeverity(value)`.
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    /// The severity's `EventExportSeverity` value.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

impl<T> EventQueue<T> {
    /// An empty queue holding at most `capacity` events.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            items: Vec::new(),
        }
    }
}

impl ExportSnapshot {
    /// Counters and queue fields compared to skip unchanged periodic health summaries.
    #[must_use]
    pub fn health_signature(&self) -> [u64; 7] {
        [
            self.aggregate.accepted,
            self.aggregate.filtered,
            self.aggregate.delivered,
            self.aggregate.dropped,
            self.aggregate.failed,
            u64::try_from(self.queue_depth).unwrap_or(u64::MAX),
            u64::try_from(self.queue_capacity).unwrap_or(u64::MAX),
        ]
    }
}
