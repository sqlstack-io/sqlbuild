//! Expected control-store schema outcomes for both supported databases.

use std::collections::BTreeSet;

use crate::store::tests::store_test_types::Backend;
use crate::store_migrations::models::StoreMigrationPlan;

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

/// What the migration contracts need to know about one store type.
pub(crate) struct StoreDialect {
    pub(crate) backend: Backend,
    pub(crate) baseline_schema: &'static str,
    pub(crate) baseline_checksum: &'static str,
    pub(crate) time_type: &'static str,
    pub(crate) shape_sql: &'static str,
    pub(crate) recreate_command: &'static str,
}

pub(crate) struct BaselineUpgradeTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static StoreDialect,
    /// The checksum a release before versioning recorded for the baseline schema.
    pub(crate) recorded_checksum: &'static str,
    pub(crate) expected_version: i64,
}

pub(crate) struct LiveWorkUpgradeTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static StoreDialect,
    /// Rows naming each broken foreign key reference; SQLite checks them only on request.
    pub(crate) foreign_key_check: &'static str,
    pub(crate) expected_version: i64,
}

pub(crate) struct TableRebuildTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static StoreDialect,
    pub(crate) expected_runs: usize,
    pub(crate) expected_version: i64,
}

pub(crate) struct MigrationRefusalTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static StoreDialect,
    pub(crate) tamper: &'static str,
    pub(crate) expected_fragments: Vec<&'static str>,
}

pub(crate) struct StoreMigrationPlanTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static StoreDialect,
    pub(crate) expected_plans: (StoreMigrationPlan, StoreMigrationPlan, Option<i64>),
    pub(crate) expected_after: (StoreMigrationPlan, StoreMigrationPlan),
}

pub(crate) struct PinnedChecksumTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static StoreDialect,
    /// The line endings Git checked the SQL files out with.
    pub(crate) line_ending: &'static str,
    /// One `(version, checksum)` per schema version: append one with each migration and never edit one.
    pub(crate) expected_checksums: &'static [(i64, &'static str)],
}

pub(crate) struct BrokenMigrationTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static StoreDialect,
    pub(crate) expected_fragment: &'static str,
    pub(crate) expected_version: Option<i64>,
}
