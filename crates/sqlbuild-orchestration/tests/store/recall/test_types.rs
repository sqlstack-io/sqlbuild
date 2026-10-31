//! Expected Recall storage outcomes for both supported databases.

use crate::store_test_types::Backend;

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
