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
