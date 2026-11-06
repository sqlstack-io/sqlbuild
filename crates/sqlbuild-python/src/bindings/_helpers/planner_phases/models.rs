//! Plan output fields gathered across the plan phases before the output is built once.

use std::collections::BTreeMap;

use pyo3::prelude::{Bound, Py, PyAny};
use pyo3::pyclass;

/// The plan output's fields and metadata entries, each set before the output is built.
#[derive(Debug)]
pub(crate) struct PlanOutputFields<'py> {
    pub(crate) fields: Vec<(&'static str, Bound<'py, PyAny>)>,
    pub(crate) metadata: Vec<(&'static str, Bound<'py, PyAny>)>,
}

/// The model changes and resolved actions after write-hash reconciliation.
#[derive(Debug)]
pub(crate) struct ChangeReconciliation<'py> {
    pub(crate) changes: Bound<'py, PyAny>,
    pub(crate) resolved_actions: Bound<'py, PyAny>,
}

/// Planned model entries and the warnings planning them raised.
#[derive(Debug)]
pub(crate) struct ModelEntryResults<'py> {
    pub(crate) entries: Vec<Bound<'py, PyAny>>,
    pub(crate) warnings: Vec<Bound<'py, PyAny>>,
}

/// The runtime, warehouse state, identities and overrides the base plan output reads.
#[derive(Debug)]
pub(crate) struct PlanOutputContext<'a, 'py> {
    pub(crate) runtime: &'a Bound<'py, PyAny>,
    pub(crate) warehouse: &'a Bound<'py, PyAny>,
    pub(crate) identities: &'a Bound<'py, PyAny>,
    pub(crate) overrides: &'a Bound<'py, PyAny>,
}

/// The execution scope, reconciled changes, planned entries and source freshness of a plan.
#[derive(Debug)]
pub(crate) struct PlannedModelEntries<'a, 'py> {
    pub(crate) execution_scope: &'a Bound<'py, PyAny>,
    pub(crate) reconciliation: &'a ChangeReconciliation<'py>,
    pub(crate) entries: ModelEntryResults<'py>,
    pub(crate) source_freshness: &'a Bound<'py, PyAny>,
}

/// The runtime, warehouse state, scopes and policies storage policy planning reads.
#[derive(Debug)]
pub(crate) struct StoragePolicyContext<'a, 'py> {
    pub(crate) runtime: &'a Bound<'py, PyAny>,
    pub(crate) warehouse: &'a Bound<'py, PyAny>,
    pub(crate) scopes: &'a Bound<'py, PyAny>,
    pub(crate) policies: &'a Bound<'py, PyAny>,
}

/// The scopes, warehouse state, identities and changes stale-selection warnings read.
#[derive(Debug)]
pub(crate) struct PlanWarningContext<'a, 'py> {
    pub(crate) scopes: &'a Bound<'py, PyAny>,
    pub(crate) warehouse: &'a Bound<'py, PyAny>,
    pub(crate) identities: &'a Bound<'py, PyAny>,
    pub(crate) stale_warning_changes: &'a Bound<'py, PyAny>,
}

/// Resolved execution environment shared by planner phases.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub(crate) struct PlannerRuntime {
    pub(crate) project: Py<PyAny>,
    pub(crate) adapter: Py<PyAny>,
    pub(crate) connection: Py<PyAny>,
    pub(crate) project_config: Py<PyAny>,
    pub(crate) local_config: Py<PyAny>,
    pub(crate) on_progress: Py<PyAny>,
    pub(crate) invocation_time: Py<PyAny>,
}

/// Warehouse snapshot, inspection relations and migration plans gathered once per plan.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub(crate) struct PlannerWarehouseState {
    pub(crate) snapshot: Py<PyAny>,
    pub(crate) inspection_relations: Py<PyAny>,
    pub(crate) migration_entries: Py<PyAny>,
    pub(crate) migration_warnings: Py<PyAny>,
    pub(crate) column_migration_entries: Py<PyAny>,
    pub(crate) column_rename_hints: Py<PyAny>,
    pub(crate) old_name_view_entries: Py<PyAny>,
    pub(crate) old_name_views: Py<PyAny>,
    pub(crate) migration_fingerprints: Py<PyAny>,
}

/// Resolved planner scopes for one plan build.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub(crate) struct PlannerScopeResolution {
    pub(crate) selected_scope: Py<PyAny>,
    pub(crate) stale_warning_scope: Py<PyAny>,
    pub(crate) inspection_scope: Py<PyAny>,
}

/// Expected version identities for inspection and stale-warning scopes.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub(crate) struct PlannerIdentityContext {
    pub(crate) version_identities: Py<PyAny>,
    pub(crate) stale_warning_identities: Py<PyAny>,
}

/// Planner identity work since the last reset, for work-count tests.
#[pyclass]
#[derive(Debug, Default)]
pub(crate) struct IdentityWork {
    pub(crate) identity_builds: u64,
    pub(crate) stale_detections: BTreeMap<String, u64>,
}
