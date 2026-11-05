use crate::store::tests::store_test_types::Backend;

pub(crate) struct SessionTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_sessions: usize,
    pub(crate) expected_peer_attempts: usize,
}
