use crate::store::tests::store_test_types::Backend;

pub(crate) struct SensorTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_runs: usize,
}
