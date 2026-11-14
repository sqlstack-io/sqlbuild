//! Expected control-store schema outcomes for both supported databases.

use std::collections::BTreeSet;

use crate::store::tests::store_test_types::Backend;

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
