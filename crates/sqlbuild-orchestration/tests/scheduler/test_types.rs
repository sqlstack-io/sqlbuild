//! Leadership scenarios run independently on both supported engines.

use crate::store_test_types::Backend;

#[derive(Debug)]
pub(crate) struct LeadershipTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_epoch: i64,
}
