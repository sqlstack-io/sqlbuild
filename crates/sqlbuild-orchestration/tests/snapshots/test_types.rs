//! Cases that alter a disposable schema to verify startup rejection.

use crate::store::tests::store_test_types::Backend;

#[derive(Debug)]
pub(super) struct SchemaRejectionCase {
    pub(super) description: &'static str,
    pub(super) update_sql: &'static str,
}

#[derive(Debug)]
pub(super) struct SnapshotTestCase {
    pub(super) description: &'static str,
    pub(super) backend: Backend,
    pub(super) expected_generation: i64,
}

#[derive(Debug)]
pub(super) struct RecoveryTestCase {
    pub(super) description: &'static str,
    pub(super) expected_generation: i64,
}

#[derive(Debug)]
pub(super) struct SnapshotRetentionTestCase {
    pub(super) description: &'static str,
    pub(super) backend: Backend,
    pub(super) expected_generations: &'static [i64],
}

#[derive(Debug)]
pub(super) struct ConnectionDetailsTestCase {
    pub(super) description: &'static str,
    pub(super) adapter: &'static str,
    pub(super) settings: serde_json::Value,
    pub(super) expected_details: serde_json::Value,
}

#[derive(Debug)]
pub(super) struct WholeSnapshotReadTestCase {
    pub(super) description: &'static str,
    pub(super) backend: Backend,
    /// Nodes `current_snapshot`, the shared read and a one-node selection return, and the shared copies two concurrent reads hold.
    pub(super) expected_reads: WholeSnapshotReads,
}

/// What each snapshot reader returned for the two-node test snapshot.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct WholeSnapshotReads {
    pub(super) current_nodes: usize,
    pub(super) current_sources: usize,
    pub(super) current_documents: usize,
    pub(super) shared_nodes: usize,
    pub(super) selected_nodes: usize,
    pub(super) one_shared_copy: bool,
}

#[derive(Debug)]
pub(super) struct StoredBodyTestCase {
    pub(super) description: &'static str,
    pub(super) stored: serde_json::Value,
    pub(super) expected_decoded: Result<&'static [u8], &'static str>,
}

#[derive(Debug)]
pub(super) struct DecodeBudgetTestCase {
    pub(super) description: &'static str,
    pub(super) backend: Backend,
    /// Whether the read finished while every MiB of the budget was held elsewhere, and how many nodes it read once freed.
    pub(super) expected_wait: (bool, usize),
}
