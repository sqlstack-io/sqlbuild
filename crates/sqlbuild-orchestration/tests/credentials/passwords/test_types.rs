use crate::store::tests::store_test_types::Backend;

pub(crate) struct PasswordTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_audits: usize,
}
