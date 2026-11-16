//! Public records of planner strategy.

use crate::graph::models::NativeProjectGraph;
use crate::native_host::models::ModelHandle;
use crate::native_host::models::ModelLookup;
use crate::planner_strategy::types::FixtureKey;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::pyclass;
use pyo3::types::PyAny;
use sqlbuild_analysis::planner_fixtures::models::FixtureColumn;
use sqlbuild_analysis::planner_fixtures::models::FixtureReadCache;
use sqlbuild_analysis::planner_fixtures::models::FixtureTypeProjection;
use sqlbuild_analysis::required_columns::models::ColumnRead;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

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
    pub(crate) relations: HashMap<FixtureKey, FixtureRelation>,
    pub authoritative_columns: HashMap<FixtureKey, BTreeSet<String>>,
    pub expected_types: HashMap<FixtureKey, Vec<(String, String)>>,
    pub fixture_inferences: Py<PyAny>,
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
    pub model: Bound<'py, PyAny>,
    pub adapter: Bound<'py, PyAny>,
    pub snapshot: Bound<'py, PyAny>,
    pub change_result: Bound<'py, PyAny>,
    pub action: Bound<'py, PyAny>,
    pub reason: Bound<'py, PyAny>,
    pub materialization_type: Bound<'py, PyAny>,
    pub resolved_sql: Bound<'py, PyAny>,
    pub sql_analysis_enabled: bool,
    pub hooks: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub cursor_column: Bound<'py, PyAny>,
    pub cursor_input_relations: Bound<'py, PyAny>,
    pub runtime_owned_cursor_bounds: bool,
    pub cursor_bounds: Bound<'py, PyAny>,
    pub microbatch_range: Bound<'py, PyAny>,
    pub empty_cursor_inputs: Bound<'py, PyAny>,
    pub waiting_on_empty_inputs: bool,
    pub backfill: Bound<'py, PyAny>,
    pub cursor_overrides: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub cursor_configs: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub invocation_time: Bound<'py, PyAny>,
}

/// The project lookups that resolve one model's cursor inputs.
#[derive(Debug)]
pub struct CursorInputLookups<'py> {
    pub models_by_name: ModelLookup,
    pub functions_by_name: Bound<'py, PyAny>,
    pub source_map: Bound<'py, PyAny>,
    pub source_warehouse_columns: Bound<'py, PyAny>,
    pub model_locations: Bound<'py, PyAny>,
    pub seed_locations: Bound<'py, PyAny>,
    pub runtime_cursor_producer_names: Bound<'py, PyAny>,
}

/// The relations and lineage that render and schedule selected audits.
#[derive(Debug)]
pub struct AuditPlanning<'py> {
    pub model_locations: Bound<'py, PyAny>,
    pub seed_locations: Bound<'py, PyAny>,
    pub source_map: Bound<'py, PyAny>,
    pub adapter: Bound<'py, PyAny>,
    pub model_materializations: Bound<'py, PyAny>,
    pub lineage: Py<NativeProjectGraph>,
}
