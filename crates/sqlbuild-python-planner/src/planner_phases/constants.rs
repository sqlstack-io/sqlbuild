//! Public constants of planner phases.

use crate::planner_phases::_helpers::inputs::getters::read_planner_identity_context;
use crate::planner_phases::_helpers::inputs::getters::read_planner_runtime;
use crate::planner_phases::_helpers::inputs::getters::read_planner_scope_resolution;
use crate::planner_phases::_helpers::inputs::getters::read_planner_warehouse_state;
use crate::planner_phases::_helpers::inputs::getters::read_retained_scope;
use crate::planner_phases::models::PlannerIdentityContext;
use crate::planner_phases::models::PlannerRuntime;
use crate::planner_phases::models::PlannerWarehouseState;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;
use sqlbuild_python_project::planner_scope::models::PlannerScopeResolution;
use sqlbuild_python_project::planner_scope::models::RetainedScope;

pub(crate) const SNOWFLAKE_ADAPTER: &str = "snowflake";

pub(crate) const BIGQUERY_ADAPTER: &str = "bigquery";

pub(crate) const TRANSIENT_TABLE_TYPE: &str = "transient";

pub(crate) const PERMANENT_TABLE_TYPE: &str = "permanent";

pub(crate) const DECREASE_WARNING: &str =
    "Retention decreases are irreversible once warehouse history expires.";

pub(crate) const DOWNGRADE_HISTORY_DAYS: i64 = 90;

pub(crate) const FULL_REFRESH_DISABLED_REBUILD_CODE: &str = "S203";

/// A FULL backfill of a non-microbatch model whose SQL reads the cursor intrinsics.
pub(crate) const FULL_BACKFILL_WITHOUT_INTERVAL_CODE: &str = "S204";

pub(crate) const MISSING_UPSTREAM_CODE: &str = "S301";

pub(crate) const RETENTION_DECREASE: &str = "decrease";

pub(crate) const RETENTION_INCREASE: &str = "increase";

pub(crate) const RETENTION_MATCH: &str = "match";

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<RetainedScope>,
        read: read_retained_scope,
    },
    GetterEntry {
        class: class_of::<PlannerIdentityContext>,
        read: read_planner_identity_context,
    },
    GetterEntry {
        class: class_of::<PlannerRuntime>,
        read: read_planner_runtime,
    },
    GetterEntry {
        class: class_of::<PlannerScopeResolution>,
        read: read_planner_scope_resolution,
    },
    GetterEntry {
        class: class_of::<PlannerWarehouseState>,
        read: read_planner_warehouse_state,
    },
];
/// A step's share of the plan's total time is shown as a percentage.
pub const PERCENT: f64 = 100.0;
/// Plan document timings are rounded to the millisecond.
pub const MILLISECONDS: f64 = 1000.0;
/// The planner policy, and the plan field, saying audit failures hold nothing back.
pub const IGNORE_AUDIT_FAILURES: &str = "ignore_audit_failures";
/// The plan field holding the planned audits.
pub const AUDIT_ENTRIES: &str = "audit_entries";
/// An audit plan entry's field saying its failure holds nothing back.
pub const IGNORE_FAILURE: &str = "ignore_failure";

/// Discovering the project's files and Python nodes; a location planning its published project skips it.
pub const DISCOVERING: &str = "Discovering the project";

/// Compiling the project before planning; a location planning its published project skips it.
pub const COMPILING: &str = "Compiling the project";

/// Evaluating the configured rules; skipped where the rules ran when the project loaded.
pub const EVALUATING_RULES: &str = "Evaluating rules";

/// Opening the warehouse connection; a plan from a synced state opens none.
pub const CONNECTING: &str = "Connecting to the warehouse";

/// The warehouse snapshot, model migrations and source relations, live or from a synced state.
pub const READING_WAREHOUSE: &str = "Reading warehouse state";

/// Model identities, buildability and the changes each selected node has.
pub const DETECTING_CHANGES: &str = "Detecting changes";

/// Observing selected sources' freshness and comparing it to recorded state.
pub const SOURCE_FRESHNESS: &str = "Checking source freshness";

/// Reconciling changes into actions and building each node's plan entry.
pub const BUILDING_ENTRIES: &str = "Building plan entries";

/// Waiting for SQL test planning, which runs alongside the other steps.
pub const TEST_FIXTURES: &str = "Preparing test fixtures";

/// Storage policies, warnings and metadata around the plan entries.
pub const ASSEMBLING: &str = "Assembling the plan";

/// `--changed`: reading recorded consumption to decide what runs.
pub const BUILD_HISTORY: &str = "Reading build history";

/// Rendering the plan as text or JSON.
pub const WRITING: &str = "Writing the plan";

/// Time inside the plan that no step covers.
pub const OTHER: &str = "Other";
/// The planner override asking every model to rebuild from scratch.
pub const FULL_REFRESH_KEY: &str = "full_refresh";
/// The planner override naming the models the named jobs rebuild from scratch.
pub const FULL_REFRESH_MODELS_KEY: &str = "full_refresh_models";
/// The planner override naming the functions the named jobs recreate.
pub const FULL_REFRESH_FUNCTIONS_KEY: &str = "full_refresh_functions";
