//! Public records of planner phases.

use pyo3::prelude::{Bound, Py, PyAny};
use pyo3::pyclass;
use std::collections::BTreeMap;

/// The plan output's fields and metadata entries, each set before the output is built.
#[derive(Debug)]
pub struct PlanOutputFields<'py> {
    pub fields: Vec<(&'static str, Bound<'py, PyAny>)>,
    pub metadata: Vec<(&'static str, Bound<'py, PyAny>)>,
}

/// The model changes and resolved actions after write-hash reconciliation.
#[derive(Debug)]
pub struct ChangeReconciliation<'py> {
    pub changes: Bound<'py, PyAny>,
    pub resolved_actions: Bound<'py, PyAny>,
}

/// Planned model entries and the warnings planning them raised.
#[derive(Debug)]
pub struct ModelEntryResults<'py> {
    pub entries: Vec<Bound<'py, PyAny>>,
    pub warnings: Vec<Bound<'py, PyAny>>,
}

/// The runtime, warehouse state, identities and overrides the base plan output reads.
#[derive(Debug)]
pub struct PlanOutputContext<'a, 'py> {
    pub runtime: &'a Bound<'py, PyAny>,
    pub warehouse: &'a Bound<'py, PyAny>,
    pub identities: &'a Bound<'py, PyAny>,
    pub overrides: &'a Bound<'py, PyAny>,
}

/// The execution scope, reconciled changes, planned entries and source freshness of a plan.
#[derive(Debug)]
pub struct PlannedModelEntries<'a, 'py> {
    pub execution_scope: &'a Bound<'py, PyAny>,
    pub(crate) reconciliation: &'a ChangeReconciliation<'py>,
    pub entries: ModelEntryResults<'py>,
    pub source_freshness: &'a Bound<'py, PyAny>,
}

/// The runtime, warehouse state, scopes and policies storage policy planning reads.
#[derive(Debug)]
pub struct StoragePolicyContext<'a, 'py> {
    pub runtime: &'a Bound<'py, PyAny>,
    pub warehouse: &'a Bound<'py, PyAny>,
    pub scopes: &'a Bound<'py, PyAny>,
    pub policies: &'a Bound<'py, PyAny>,
}

/// The scopes, warehouse state, identities and changes stale-selection warnings read.
#[derive(Debug)]
pub struct PlanWarningContext<'a, 'py> {
    pub scopes: &'a Bound<'py, PyAny>,
    pub warehouse: &'a Bound<'py, PyAny>,
    pub identities: &'a Bound<'py, PyAny>,
    pub stale_warning_changes: &'a Bound<'py, PyAny>,
}

/// Resolved execution environment shared by planner phases.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub struct PlannerRuntime {
    pub project: Py<PyAny>,
    pub adapter: Py<PyAny>,
    pub connection: Py<PyAny>,
    pub project_config: Py<PyAny>,
    pub local_config: Py<PyAny>,
    pub on_progress: Py<PyAny>,
    pub invocation_time: Py<PyAny>,
}

/// Warehouse snapshot, inspection relations and migration plans gathered once per plan.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub struct PlannerWarehouseState {
    pub snapshot: Py<PyAny>,
    pub inspection_relations: Py<PyAny>,
    pub migration_entries: Py<PyAny>,
    pub migration_warnings: Py<PyAny>,
    pub column_migration_entries: Py<PyAny>,
    pub column_rename_hints: Py<PyAny>,
    pub old_name_view_entries: Py<PyAny>,
    pub old_name_views: Py<PyAny>,
    pub migration_fingerprints: Py<PyAny>,
}

/// Expected version identities for inspection and stale-warning scopes.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub struct PlannerIdentityContext {
    pub version_identities: Py<PyAny>,
    pub stale_warning_identities: Py<PyAny>,
}

/// Planner identity work since the last reset, for work-count tests.
#[pyclass]
#[derive(Debug, Default)]
pub(crate) struct IdentityWork {
    pub identity_builds: u64,
    pub stale_detections: BTreeMap<String, u64>,
}
