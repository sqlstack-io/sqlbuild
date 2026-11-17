use crate::event_export::models::ExportSeverity;

/// One catalogued event type and the export dimensions it maps to.
pub(crate) struct PolicyTestCase {
    pub(crate) description: &'static str,
    pub(crate) event_type: &'static str,
    pub(crate) terminal: bool,
    pub(crate) expected_kind: &'static str,
    pub(crate) expected_severity: ExportSeverity,
    pub(crate) expected_priority: u8,
}

/// One exception class name and its sanitized diagnostic form.
pub(crate) struct SanitizationTestCase {
    pub(crate) description: &'static str,
    pub(crate) name: String,
    pub(crate) expected_name: String,
}

/// A required and optional minimum severity and the stricter of the two.
pub(crate) struct StricterSeverityTestCase {
    pub(crate) description: &'static str,
    pub(crate) severities: (ExportSeverity, Option<ExportSeverity>),
    pub(crate) expected_severity: ExportSeverity,
}

/// One full single-slot queue receiving a second event.
pub(crate) struct DisplacementTestCase {
    pub(crate) description: &'static str,
    pub(crate) priorities: (u8, u8),
    pub(crate) expected_inserted: bool,
    pub(crate) expected_displaced: Option<u64>,
    pub(crate) expected_retained: u64,
}

/// Queued `(priority, invocation terminal)` events and their delivery order by sequence.
pub(crate) struct DeliveryOrderTestCase {
    pub(crate) description: &'static str,
    pub(crate) events: &'static [(u8, bool)],
    pub(crate) expected_sequences: &'static [u64],
}

/// Queued events as `(priority, invocation start, invocation)`, and the order they are taken in.
pub(crate) struct InvocationStartOrderTestCase {
    pub(crate) description: &'static str,
    pub(crate) events: &'static [(u8, bool, &'static str)],
    pub(crate) expected_sequences: &'static [u64],
}

/// One event routed across sink filters, and the resulting accounting.
pub(crate) struct RoutingTestCase {
    pub(crate) description: &'static str,
    pub(crate) policy: (&'static str, ExportSeverity),
    pub(crate) filters: &'static [(&'static [&'static str], ExportSeverity)],
    pub(crate) expected_eligible: &'static [usize],
    pub(crate) expected_filtered: u64,
}

/// Per-exporter `(accepted, delivered, failed, dropped)` counts and the snapshot completion.
pub(crate) struct SnapshotTestCase {
    pub(crate) description: &'static str,
    pub(crate) counts: &'static [(u64, u64, u64, u64)],
    pub(crate) queue_depth: usize,
    pub(crate) drained: bool,
    pub(crate) expected_accepted: u64,
    pub(crate) expected_flush_complete: bool,
}

/// A full queue of `(priority, outcome)` events receiving one more, and what it keeps.
pub(crate) struct OutcomeAdmissionTestCase {
    pub(crate) description: &'static str,
    pub(crate) queued: &'static [(u8, bool)],
    pub(crate) incoming: (u8, bool),
    pub(crate) expected_inserted: bool,
    pub(crate) expected_displaced: Option<u64>,
    pub(crate) expected_sequences: &'static [u64],
}

/// Queued `(priority, outcome)` events and what the shutdown reserve keeps and drops, by sequence.
pub(crate) struct OutcomeReserveTestCase {
    pub(crate) description: &'static str,
    pub(crate) queued: &'static [(u8, bool)],
    pub(crate) expected_kept: &'static [u64],
    pub(crate) expected_dropped: &'static [u64],
}
