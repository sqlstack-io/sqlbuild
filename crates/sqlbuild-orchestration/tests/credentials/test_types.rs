use std::sync::{Arc, Mutex, atomic::AtomicU16};

use serde_json::Value;

pub(crate) use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::store_test_types::Backend;

pub(crate) struct CredentialTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_hash_length: usize,
}

pub(crate) struct AdministratorTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_administrators: usize,
}

pub(crate) struct AdministratorRecoveryTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_recoveries: usize,
}

pub(crate) struct AuditTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_page_length: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct AuditWebhookReceiver {
    pub(crate) requests: Arc<Mutex<Vec<Value>>>,
    pub(crate) status: Arc<AtomicU16>,
}
