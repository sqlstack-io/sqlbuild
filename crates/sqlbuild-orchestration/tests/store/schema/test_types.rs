//! Expected control-store schema outcomes for both supported databases.

use std::collections::BTreeSet;

use crate::store::tests::store_test_types::Backend;

pub(crate) struct CrashRecoveryTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_acknowledged: usize,
}

pub(crate) struct WalCheckpointTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_acknowledged: usize,
    pub(crate) expected_wal_bytes: u64,
}

pub(crate) struct WalGrowthTestCase {
    pub(crate) description: &'static str,
    pub(crate) commits: usize,
    pub(crate) row_bytes: usize,
    pub(crate) expected_max_wal_bytes: u64,
}

pub(crate) struct ForeignKeyIndexTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_uncovered: Vec<(String, BTreeSet<String>)>,
}

pub(crate) struct StoreUpgradeTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) list_indexes: &'static str,
    pub(crate) expected_indexes: Vec<&'static str>,
}
