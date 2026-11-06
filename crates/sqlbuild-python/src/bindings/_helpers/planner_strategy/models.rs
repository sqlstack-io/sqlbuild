//! Extension-owned fixture analysis reuse and the project-wide fixture planning context.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use pyo3::prelude::{Bound, Py};
use pyo3::pyclass;
use pyo3::types::PyAny;
use sqlbuild_analysis::planner_fixtures::models::{
    FixtureColumn, FixtureReadCache, FixtureTypeProjection,
};
use sqlbuild_analysis::required_columns::models::ColumnRead;

use crate::bindings::_helpers::graph::project_graph::NativeProjectGraph;
use crate::bindings::_helpers::native_host::model_lookup::ModelHandle;
use crate::bindings::_helpers::native_host::model_lookup::ModelLookup;
use crate::bindings::_helpers::planner_strategy::types::FixtureKey;

/// Per-extension cache replacing the Python fixture resolver's LRU.
#[pyclass]
#[derive(Debug, Default)]
pub(crate) struct FixtureReadCacheHost {
    pub(crate) cache: FixtureReadCache,
    /// Fixture planning contexts built since the last reset.
    pub(crate) context_builds: u64,
    /// SQL-test model chain batches resolved since the last reset.
    pub(crate) chain_resolutions: u64,
}

/// One relation's known fixture columns and whether they form an authoritative shape.
#[derive(Clone, Debug)]
pub(crate) struct FixtureRelation {
    pub(crate) columns: Vec<FixtureColumn>,
    pub(crate) authoritative: bool,
}

/// One model's high-confidence lineage reads and direct type projections.
#[derive(Debug)]
pub(crate) struct ModelFixtureFacts {
    pub(crate) reads: Vec<ColumnRead>,
    pub(crate) projections: Vec<FixtureTypeProjection>,
}

/// Project-wide fixture metadata shared across test and scenario planning.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub(crate) struct RelationFixturePlanningContext {
    pub(crate) models_by_name: HashMap<String, ModelHandle>,
    pub(crate) relations: HashMap<FixtureKey, FixtureRelation>,
    pub(crate) authoritative_columns: HashMap<FixtureKey, BTreeSet<String>>,
    pub(crate) expected_types: HashMap<FixtureKey, Vec<(String, String)>>,
    pub(crate) fixture_inferences: Py<PyAny>,
    pub(crate) model_facts: Mutex<HashMap<String, Arc<ModelFixtureFacts>>>,
}

/// The expected column types of one completion: its chain's propagated types over the context's.
#[derive(Debug)]
pub(crate) struct ExpectedTypes<'a> {
    pub(crate) overlay: Vec<(FixtureKey, Vec<(String, String)>)>,
    pub(crate) base: &'a HashMap<FixtureKey, Vec<(String, String)>>,
}

/// One model's resolved plan decisions, assembled into its `ModelPlanEntry`.
#[derive(Debug)]
pub(crate) struct ModelEntryDecisions<'py> {
    pub(crate) model: Bound<'py, PyAny>,
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) snapshot: Bound<'py, PyAny>,
    pub(crate) change_result: Bound<'py, PyAny>,
    pub(crate) action: Bound<'py, PyAny>,
    pub(crate) reason: Bound<'py, PyAny>,
    pub(crate) materialization_type: Bound<'py, PyAny>,
    pub(crate) resolved_sql: Bound<'py, PyAny>,
    pub(crate) sql_analysis_enabled: bool,
    pub(crate) hooks: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub(crate) cursor_column: Bound<'py, PyAny>,
    pub(crate) cursor_input_relations: Bound<'py, PyAny>,
    pub(crate) runtime_owned_cursor_bounds: bool,
    pub(crate) cursor_bounds: Bound<'py, PyAny>,
    pub(crate) microbatch_range: Bound<'py, PyAny>,
    pub(crate) empty_cursor_inputs: Bound<'py, PyAny>,
    pub(crate) waiting_on_empty_inputs: bool,
    pub(crate) backfill: Bound<'py, PyAny>,
    pub(crate) cursor_overrides: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub(crate) cursor_configs: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub(crate) invocation_time: Bound<'py, PyAny>,
}

/// The project lookups that resolve one model's cursor inputs.
#[derive(Debug)]
pub(crate) struct CursorInputLookups<'py> {
    pub(crate) models_by_name: ModelLookup,
    pub(crate) functions_by_name: Bound<'py, PyAny>,
    pub(crate) source_map: Bound<'py, PyAny>,
    pub(crate) source_warehouse_columns: Bound<'py, PyAny>,
    pub(crate) model_locations: Bound<'py, PyAny>,
    pub(crate) seed_locations: Bound<'py, PyAny>,
    pub(crate) runtime_cursor_producer_names: Bound<'py, PyAny>,
}

/// The relations and lineage that render and schedule selected audits.
#[derive(Debug)]
pub(crate) struct AuditPlanning<'py> {
    pub(crate) model_locations: Bound<'py, PyAny>,
    pub(crate) seed_locations: Bound<'py, PyAny>,
    pub(crate) source_map: Bound<'py, PyAny>,
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) model_materializations: Bound<'py, PyAny>,
    pub(crate) lineage: Py<NativeProjectGraph>,
}
