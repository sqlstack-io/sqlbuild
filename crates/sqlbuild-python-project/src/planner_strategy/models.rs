//! Public records of planner strategy.

use crate::graph::models::NativeProjectGraph;
use crate::native_host::models::ModelHandle;
use crate::native_host::models::ModelLookup;
use crate::planner_strategy::types::FixtureKey;
use pyo3::prelude::{Bound, Py};
use pyo3::pyclass;
use pyo3::types::PyDict;
use sqlbuild_analysis::planner_fixtures::models::{
    FixtureColumn, FixtureReadCache, FixtureTypeProjection,
};
use sqlbuild_analysis::required_columns::models::ColumnRead;
use sqlbuild_python_base::native_host::models::Dialect;
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

/// Completed fixture SQL and inferred columns by Python fixture key, expected types and diagnostics.
#[derive(Debug)]
pub struct FixtureCompletion<'py, 'a> {
    pub sql_by_key: Bound<'py, PyDict>,
    pub inferred_by_fixture: Bound<'py, PyDict>,
    pub(crate) expected_types: ExpectedTypes<'a>,
    pub diagnostics: Vec<Bound<'py, pyo3::prelude::PyAny>>,
}

/// Per-extension cache replacing the Python fixture resolver's LRU.
#[pyclass]
#[derive(Debug, Default)]
pub(crate) struct FixtureReadCacheHost {
    pub cache: FixtureReadCache,
    /// Fixture planning contexts built since the last reset.
    pub context_builds: u64,
    /// SQL-test model chain batches resolved since the last reset.
    pub chain_resolutions: u64,
}

/// One relation's known fixture columns and whether they form an authoritative shape.
#[derive(Clone, Debug)]
pub struct FixtureRelation {
    pub columns: Vec<FixtureColumn>,
    pub authoritative: bool,
}

/// One model's high-confidence lineage reads and direct type projections.
#[derive(Debug)]
pub(crate) struct ModelFixtureFacts {
    pub reads: Vec<ColumnRead>,
    pub projections: Vec<FixtureTypeProjection>,
}

/// Project-wide fixture metadata shared across test and scenario planning.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct RelationFixturePlanningContext {
    pub models_by_name: HashMap<String, ModelHandle>,
    pub relations: HashMap<FixtureKey, FixtureRelation>,
    pub authoritative_columns: HashMap<FixtureKey, BTreeSet<String>>,
    pub expected_types: HashMap<FixtureKey, Vec<(String, String)>>,
    pub fixture_inferences: Py<pyo3::types::PyAny>,
    pub(crate) model_facts: Mutex<HashMap<String, Arc<ModelFixtureFacts>>>,
}

/// The expected column types of one completion: its chain's propagated types over the context's.
#[derive(Debug)]
pub(crate) struct ExpectedTypes<'a> {
    pub overlay: Vec<(FixtureKey, Vec<(String, String)>)>,
    pub base: &'a HashMap<FixtureKey, Vec<(String, String)>>,
}

/// One model's resolved plan decisions, assembled into its `ModelPlanEntry`.
#[derive(Debug)]
pub struct ModelEntryDecisions<'py> {
    pub model: Bound<'py, pyo3::types::PyAny>,
    pub dialect: Bound<'py, Dialect>,
    pub snapshot: Bound<'py, pyo3::types::PyAny>,
    pub change_result: Bound<'py, pyo3::types::PyAny>,
    pub action: Bound<'py, pyo3::types::PyAny>,
    pub reason: Bound<'py, pyo3::types::PyAny>,
    pub materialization_type: Bound<'py, pyo3::types::PyAny>,
    pub resolved_sql: Bound<'py, pyo3::types::PyAny>,
    pub sql_analysis_enabled: bool,
    pub hooks: (
        Bound<'py, pyo3::types::PyAny>,
        Bound<'py, pyo3::types::PyAny>,
    ),
    pub cursor_column: Bound<'py, pyo3::types::PyAny>,
    pub cursor_input_relations: Bound<'py, pyo3::types::PyAny>,
    pub runtime_owned_cursor_bounds: bool,
    pub cursor_bounds: Bound<'py, pyo3::types::PyAny>,
    pub microbatch_range: Bound<'py, pyo3::types::PyAny>,
    pub empty_cursor_inputs: Bound<'py, pyo3::types::PyAny>,
    pub waiting_on_empty_inputs: bool,
    pub backfill: Bound<'py, pyo3::types::PyAny>,
    pub cursor_overrides: (
        Bound<'py, pyo3::types::PyAny>,
        Bound<'py, pyo3::types::PyAny>,
    ),
    pub cursor_configs: (
        Bound<'py, pyo3::types::PyAny>,
        Bound<'py, pyo3::types::PyAny>,
    ),
    pub invocation_time: Bound<'py, pyo3::types::PyAny>,
}

/// The project lookups that resolve one model's cursor inputs.
#[derive(Debug)]
pub struct CursorInputLookups<'py> {
    pub models_by_name: ModelLookup,
    pub functions_by_name: Bound<'py, pyo3::types::PyAny>,
    pub source_map: Bound<'py, pyo3::types::PyAny>,
    pub source_warehouse_columns: Bound<'py, pyo3::types::PyAny>,
    pub model_locations: Bound<'py, pyo3::types::PyAny>,
    pub seed_locations: Bound<'py, pyo3::types::PyAny>,
    pub runtime_cursor_producer_names: Bound<'py, pyo3::types::PyAny>,
}

/// The relations and lineage that render and schedule selected audits.
#[derive(Debug)]
pub struct AuditPlanning<'py> {
    pub model_locations: Bound<'py, pyo3::types::PyAny>,
    pub seed_locations: Bound<'py, pyo3::types::PyAny>,
    pub source_map: Bound<'py, pyo3::types::PyAny>,
    pub dialect: Bound<'py, Dialect>,
    pub model_materializations: Bound<'py, pyo3::types::PyAny>,
    pub lineage: Py<NativeProjectGraph>,
}
