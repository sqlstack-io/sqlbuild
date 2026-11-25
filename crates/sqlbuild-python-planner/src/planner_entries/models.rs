//! Public records of planner entries.

use crate::planner_phases::models::FullRefreshRequest;
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, Py, PyAny};
use pyo3::pyclass;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_project::native_host::models::ModelLookup;

/// What resolving relation locations and source metadata for one scope reads.
#[derive(Debug)]
pub struct RelationsRequest<'py> {
    pub(crate) project: Bound<'py, PyAny>,
    pub(crate) dialect: Bound<'py, Dialect>,
    /// The warehouse reads, or `None` to resolve from canonical static state only.
    pub(crate) reads: Option<Bound<'py, PlannerReads>>,
    pub(crate) scope: Bound<'py, PyAny>,
    pub(crate) deferral: Bound<'py, PyAny>,
    pub(crate) project_config: Bound<'py, PyAny>,
    pub(crate) local_config: Bound<'py, PyAny>,
    pub(crate) known_source_columns: Bound<'py, PyAny>,
    pub(crate) relation_keys: Bound<'py, PyAny>,
}

/// What building model plan entries for one execution scope reads.
#[derive(Debug)]
pub struct PlanEntriesRequest<'py> {
    pub(crate) project: Bound<'py, PyAny>,
    pub(crate) dialect: Bound<'py, Dialect>,
    pub(crate) scope: Bound<'py, PyAny>,
    pub(crate) snapshot: Bound<'py, PyAny>,
    pub(crate) relations: Bound<'py, PyAny>,
    pub(crate) resolved_models: Bound<'py, PyAny>,
    pub(crate) cursor_overrides: Bound<'py, PyAny>,
    pub(crate) full_refresh: FullRefreshRequest,
    pub(crate) build_inputs: Bound<'py, PyAny>,
    /// The effective worker limit execution will use.
    pub(crate) concurrency: i64,
}

/// What assembling the plan output's fields reads.
#[derive(Debug)]
pub struct PlanOutputInputs<'py> {
    pub(crate) project: Bound<'py, PyAny>,
    pub(crate) dialect: Bound<'py, Dialect>,
    pub(crate) scope: Bound<'py, PyAny>,
    pub(crate) snapshot: Bound<'py, PyAny>,
    pub(crate) relations: Bound<'py, PyAny>,
    pub(crate) changes: Bound<'py, PyAny>,
    pub(crate) model_entries: Vec<Bound<'py, PyAny>>,
    pub(crate) model_warnings: Vec<Bound<'py, PyAny>>,
    pub(crate) reload_sources: Bound<'py, PyAny>,
    pub(crate) extras: Bound<'py, PyAny>,
    /// End-of-build audits that won't run, by name with the reads that don't exist.
    pub(crate) audits_not_run: std::collections::BTreeMap<String, Vec<String>>,
}

/// Resolved relation and source inputs for plan entry construction.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub struct PlannerRelationsContext {
    pub model_locations: Py<PyAny>,
    pub seed_locations: Py<PyAny>,
    pub function_locations: Py<PyAny>,
    pub source_map: Py<PyAny>,
    pub source_read_map: Py<PyAny>,
    pub python_source_read_map: Py<PyAny>,
    pub source_warehouse_columns: Py<PyAny>,
    pub star_exclude_keyword: Py<PyAny>,
    pub listed_source_names: Py<PyAny>,
}

/// Relation lookups and source columns for building one model plan entry.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub struct ModelPlanContext {
    pub model_locations: Py<PyAny>,
    pub models_by_name: ModelLookup,
    pub seed_locations: Py<PyAny>,
    pub function_locations: Py<PyAny>,
    pub source_map: Py<PyAny>,
    pub source_warehouse_columns: Py<PyAny>,
    pub star_exclude_keyword: Py<PyAny>,
    pub functions_by_name: Py<PyAny>,
    pub runtime_cursor_producer_names: Py<PyAny>,
    pub future_cursor_config: Py<PyAny>,
    pub start_cursor_config: Py<PyAny>,
    pub invocation_time: Py<PyAny>,
}

/// Blocked models and cursor overrides for plan entry building.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub struct PlanEntryBuildInputs {
    pub source_freshness_blocked_model_names: Py<PyAny>,
    pub external_blocked_model_names: Py<PyAny>,
    pub start_cursor_override: Py<PyAny>,
    pub end_cursor_override: Py<PyAny>,
    pub future_cursor_config: Py<PyAny>,
    pub start_cursor_config: Py<PyAny>,
    pub invocation_time: Py<PyAny>,
    pub max_microbatches: Py<PyAny>,
    pub max_microbatches_is_override: bool,
    pub microbatch_limit_action: Py<PyAny>,
}

/// One model's turn under an explicit cursor window: planned, with a clamp warning when the window started before its floor, or skipped because the window ends at or before it.
pub(crate) enum WindowTurn<'py> {
    Planned(
        (Bound<'py, PyAny>, Bound<'py, PyAny>),
        Option<Bound<'py, PyAny>>,
    ),
    /// The skip row, and the floor and window that skipped it.
    Skipped(Bound<'py, PyAny>, (String, String)),
}

/// What an explicit cursor window did across the selection: how many cursor models planned, and which it skipped (`model (cursor_start floor)`, and the window).
#[derive(Debug, Default)]
pub(crate) struct WindowTally {
    pub(crate) cursor_models_planned: usize,
    pub(crate) window_skips: Vec<(String, String)>,
}
