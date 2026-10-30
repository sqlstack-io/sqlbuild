//! Cases that alter a disposable schema to verify startup rejection.

use crate::store_test_types::Backend;

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
