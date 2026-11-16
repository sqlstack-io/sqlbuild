//! Public constants of planner scenario.

pub(crate) const REF: &str = "ref";

pub(crate) const SOURCE: &str = "source";

pub(crate) const SOURCE_MARKER: usize = 2;

pub(crate) const DBT_REF: &str = "dbt_ref";

pub(crate) const SCENARIO_PLAN_INVALID_HASH_PREFIX: &str = "S501";

pub(crate) const SCENARIO_PLAN_HASH_COLLISION: &str = "S502";

pub(crate) const SCENARIO_PLAN_RELATION_COLLISION: &str = "S503";

pub(crate) const SCENARIO_PLAN_GRAPH_VALIDATION: &str = "S504";

pub(crate) const SCENARIO_PLAN_UNKNOWN_SEED: &str = "S507";

pub(crate) const SCENARIO_PLAN_MISSING_FIXTURE_SQL: &str = "S508";

pub(crate) const SCENARIO_PLAN_MISSING_RELATION_TARGET: &str = "S509";

pub(crate) const SCENARIO_PLAN_INVALID_FIXTURE: &str = "S510";

pub(crate) const SCENARIO_PLAN_UNRESOLVED_RELATION_MARKER: &str = "S511";

pub(crate) const SCENARIO_PLAN_EXPECTED_COLUMNS: &str = "S512";

pub(crate) const SCENARIO_EXPECTED_COLUMNS_HELP: &str = "list every column the model outputs in the expected CTE, and remove columns it does not output";

pub(crate) const SCENARIO_PLAN_INTERNAL: &str = "S599";

/// The zero-argument cursor intrinsics a model query can call.
pub(crate) const CURSOR_INTRINSIC_NAMES: [&str; 2] = ["__cursor_start", "__cursor_end"];

pub(crate) const INCREMENTAL_MODE_KEY: &str = "incremental_mode";

pub(crate) const MICROBATCH_INCREMENTAL_MODE: &str = "microbatch";

pub(crate) const CURSOR_TYPE_KEY: &str = "cursor_type";

pub(crate) const INTEGER_CURSOR_TYPE: &str = "integer";
