//! Typed requests of plan entry construction.

use crate::bindings::_helpers::native_host::model_lookup::ModelLookup;
use pyo3::prelude::{Bound, Py, PyAny};
use pyo3::pyclass;

/// What resolving relation locations and source metadata for one scope reads.
#[derive(Debug)]
pub(crate) struct RelationsRequest<'py> {
    pub(crate) project: Bound<'py, PyAny>,
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) connection: Bound<'py, PyAny>,
    pub(crate) scope: Bound<'py, PyAny>,
    pub(crate) deferral: Bound<'py, PyAny>,
    pub(crate) project_config: Bound<'py, PyAny>,
    pub(crate) local_config: Bound<'py, PyAny>,
    pub(crate) known_source_columns: Bound<'py, PyAny>,
    pub(crate) relation_keys: Bound<'py, PyAny>,
}

/// What building model plan entries for one execution scope reads.
#[derive(Debug)]
pub(crate) struct PlanEntriesRequest<'py> {
    pub(crate) project: Bound<'py, PyAny>,
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) scope: Bound<'py, PyAny>,
    pub(crate) snapshot: Bound<'py, PyAny>,
    pub(crate) relations: Bound<'py, PyAny>,
    pub(crate) resolved_models: Bound<'py, PyAny>,
    pub(crate) cursor_overrides: Bound<'py, PyAny>,
    pub(crate) full_refresh: bool,
    pub(crate) build_inputs: Bound<'py, PyAny>,
    /// The effective worker limit execution will use.
    pub(crate) concurrency: i64,
}

/// What assembling the plan output's fields reads.
#[derive(Debug)]
pub(crate) struct PlanOutputInputs<'py> {
    pub(crate) project: Bound<'py, PyAny>,
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) scope: Bound<'py, PyAny>,
    pub(crate) snapshot: Bound<'py, PyAny>,
    pub(crate) relations: Bound<'py, PyAny>,
    pub(crate) changes: Bound<'py, PyAny>,
    pub(crate) model_entries: Vec<Bound<'py, PyAny>>,
    pub(crate) model_warnings: Vec<Bound<'py, PyAny>>,
    pub(crate) reload_sources: Bound<'py, PyAny>,
    pub(crate) extras: Bound<'py, PyAny>,
}

/// Resolved relation and source inputs for plan entry construction.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub(crate) struct PlannerRelationsContext {
    pub(crate) model_locations: Py<PyAny>,
    pub(crate) seed_locations: Py<PyAny>,
    pub(crate) function_locations: Py<PyAny>,
    pub(crate) source_map: Py<PyAny>,
    pub(crate) source_read_map: Py<PyAny>,
    pub(crate) python_source_read_map: Py<PyAny>,
    pub(crate) source_warehouse_columns: Py<PyAny>,
    pub(crate) star_exclude_keyword: Py<PyAny>,
    pub(crate) listed_source_names: Py<PyAny>,
}

/// Relation lookups and source columns for building one model plan entry.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub(crate) struct ModelPlanContext {
    pub(crate) model_locations: Py<PyAny>,
    pub(crate) models_by_name: ModelLookup,
    pub(crate) seed_locations: Py<PyAny>,
    pub(crate) function_locations: Py<PyAny>,
    pub(crate) source_map: Py<PyAny>,
    pub(crate) source_warehouse_columns: Py<PyAny>,
    pub(crate) star_exclude_keyword: Py<PyAny>,
    pub(crate) functions_by_name: Py<PyAny>,
    pub(crate) runtime_cursor_producer_names: Py<PyAny>,
    pub(crate) future_cursor_config: Py<PyAny>,
    pub(crate) start_cursor_config: Py<PyAny>,
    pub(crate) invocation_time: Py<PyAny>,
}

/// Blocked models and cursor overrides for plan entry building.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub(crate) struct PlanEntryBuildInputs {
    pub(crate) source_freshness_blocked_model_names: Py<PyAny>,
    pub(crate) external_blocked_model_names: Py<PyAny>,
    pub(crate) start_cursor_override: Py<PyAny>,
    pub(crate) end_cursor_override: Py<PyAny>,
    pub(crate) future_cursor_config: Py<PyAny>,
    pub(crate) start_cursor_config: Py<PyAny>,
    pub(crate) invocation_time: Py<PyAny>,
    pub(crate) max_microbatches: Py<PyAny>,
    pub(crate) max_microbatches_is_override: bool,
    pub(crate) microbatch_limit_action: Py<PyAny>,
}
