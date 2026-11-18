//! Table cases of warehouse state lookups.

use crate::warehouse_state::models::WarehouseStateParts;

/// A listing and the `schema.name` of each relation it returns.
pub(crate) struct RelationsTestCase {
    pub(crate) description: &'static str,
    pub(crate) database: Option<&'static str>,
    pub(crate) schemas: Option<&'static [&'static str]>,
    pub(crate) names: Option<&'static [&'static str]>,
    pub(crate) expected_relations: &'static [&'static str],
}

/// A fingerprint read of one state location and the `name@run` of each newest fingerprint.
pub(crate) struct FingerprintsTestCase {
    pub(crate) description: &'static str,
    pub(crate) database: Option<&'static str>,
    pub(crate) schema: &'static str,
    pub(crate) expected_fingerprints: &'static [&'static str],
}

/// A model and the cursor `MAX` its newest successful build observed.
pub(crate) struct TargetMaxTestCase {
    pub(crate) description: &'static str,
    pub(crate) model: &'static str,
    pub(crate) expected_max: Option<&'static str>,
}

/// A source, the SQL a plan describes it with, and the columns the state knows for it.
pub(crate) struct DescribedColumnsTestCase {
    pub(crate) description: &'static str,
    pub(crate) source: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_columns: Option<&'static [&'static str]>,
}

/// A consumption read of one state location and the execution IDs it returns.
pub(crate) struct ConsumptionTestCase {
    pub(crate) description: &'static str,
    pub(crate) database: Option<&'static str>,
    pub(crate) schema: &'static str,
    pub(crate) expected_executions: &'static [&'static str],
}

/// Facts stored as JSON and read back.
pub(crate) struct RoundTripTestCase {
    pub(crate) description: &'static str,
    pub(crate) parts: fn() -> WarehouseStateParts,
    pub(crate) expected_unchanged: bool,
}
