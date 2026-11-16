//! Public records of planner entries.

use pyo3::prelude::{Bound, Py, PyAny};
use pyo3::pyclass;
use sqlbuild_python_project::native_host::models::ModelLookup;

/// What resolving relation locations and source metadata for one scope reads.
#[derive(Debug)]
pub struct RelationsRequest<'py> {
    pub project: Bound<'py, PyAny>,
    pub adapter: Bound<'py, PyAny>,
    pub connection: Bound<'py, PyAny>,
    pub scope: Bound<'py, PyAny>,
    pub deferral: Bound<'py, PyAny>,
    pub project_config: Bound<'py, PyAny>,
    pub local_config: Bound<'py, PyAny>,
    pub known_source_columns: Bound<'py, PyAny>,
    pub relation_keys: Bound<'py, PyAny>,
}

/// What building model plan entries for one execution scope reads.
#[derive(Debug)]
pub struct PlanEntriesRequest<'py> {
    pub project: Bound<'py, PyAny>,
    pub adapter: Bound<'py, PyAny>,
    pub scope: Bound<'py, PyAny>,
    pub snapshot: Bound<'py, PyAny>,
    pub relations: Bound<'py, PyAny>,
    pub resolved_models: Bound<'py, PyAny>,
    pub cursor_overrides: Bound<'py, PyAny>,
    pub full_refresh: bool,
    pub build_inputs: Bound<'py, PyAny>,
    /// The effective worker limit execution will use.
    pub concurrency: i64,
}

/// What assembling the plan output's fields reads.
#[derive(Debug)]
pub struct PlanOutputInputs<'py> {
    pub project: Bound<'py, PyAny>,
    pub adapter: Bound<'py, PyAny>,
    pub scope: Bound<'py, PyAny>,
    pub snapshot: Bound<'py, PyAny>,
    pub relations: Bound<'py, PyAny>,
    pub changes: Bound<'py, PyAny>,
    pub model_entries: Vec<Bound<'py, PyAny>>,
    pub model_warnings: Vec<Bound<'py, PyAny>>,
    pub reload_sources: Bound<'py, PyAny>,
    pub extras: Bound<'py, PyAny>,
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
