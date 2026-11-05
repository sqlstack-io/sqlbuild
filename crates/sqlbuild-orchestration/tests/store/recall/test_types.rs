//! Expected Recall storage outcomes for both supported databases.

use crate::store::tests::store_test_types::Backend;

pub(crate) struct RecallTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_event_ids: Vec<&'static str>,
}

pub(crate) struct RecallViewTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_complete: i64,
}

pub(crate) struct CommitCancellationTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_body: &'static [u8],
}
