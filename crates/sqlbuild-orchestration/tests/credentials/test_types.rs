pub(crate) use crate::fault_types::TransactionFault;
use crate::store_test_types::Backend;

pub(crate) struct CredentialTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_hash_length: usize,
}
