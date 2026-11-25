//! Public records of planner phases.

use crate::planner_phases::_helpers::timing::timings::STEPS;
use crate::planner_phases::_helpers::timing::timings::timings_of;
use crate::planner_phases::constants::MILLISECONDS;
use crate::planner_phases::constants::PERCENT;
use crate::planner_phases::constants::{
    FULL_REFRESH_FUNCTIONS_KEY, FULL_REFRESH_KEY, FULL_REFRESH_MODELS_KEY,
};
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, Py, PyAny};
use pyo3::prelude::{PyAnyMethods, PyResult};
use pyo3::pyclass;
use serde_json::{Value, json};
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

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
    /// End-of-build audits that won't run, by name with the reads that don't exist.
    pub audits_not_run: &'a BTreeMap<String, Vec<String>>,
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
    pub(crate) project: Py<PyAny>,
    pub(crate) dialect: Py<Dialect>,
    pub(crate) reads: Py<PlannerReads>,
    pub(crate) project_config: Py<PyAny>,
    pub(crate) local_config: Py<PyAny>,
    pub(crate) on_progress: Py<PyAny>,
    pub(crate) invocation_time: Py<PyAny>,
    /// Python nodes' `(database, schema)`, where their results and fingerprints live; `None` when unknown.
    pub(crate) node_results_location: Option<(Option<String>, Option<String>)>,
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

/// While held, this thread's planning steps are recorded for one plan command.
#[derive(Debug)]
pub struct PlanTimingScope {
    started: Instant,
    previous: Option<Vec<(&'static str, f64)>>,
}

/// A plan's steps in pipeline order with `Other` last, and its total, in seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanTimings {
    pub(crate) total_seconds: f64,
    pub(crate) steps: Vec<(&'static str, f64)>,
}

impl PlanTimingScope {
    pub fn enter() -> Self {
        Self {
            started: Instant::now(),
            previous: STEPS.replace(Some(Vec::new())),
        }
    }

    /// The steps recorded so far and the time since the scope opened; `Other` is the remainder.
    pub fn timings(&self) -> PlanTimings {
        let total_seconds: f64 = self.started.elapsed().as_secs_f64();
        let recorded: Vec<(&'static str, f64)> =
            STEPS.with(|steps| steps.borrow().clone().unwrap_or_default());
        timings_of(recorded, total_seconds)
    }
}

impl Drop for PlanTimingScope {
    fn drop(&mut self) {
        STEPS.set(self.previous.take());
    }
}

impl PlanTimings {
    /// The `--timings` table: each step's seconds and share of the total, then the total.
    pub fn table(&self) -> String {
        const HEADER: &str = "Plan timings";
        let name_width: usize = self
            .steps
            .iter()
            .map(|(name, _)| name.len() + 2)
            .max()
            .unwrap_or(0)
            .max(HEADER.len());
        let mut lines: Vec<String> = vec![format!(
            "{HEADER:<name_width$}  {:>7}  {:>5}",
            "seconds", "share"
        )];
        let row_width: usize = name_width - 2;
        for (name, seconds) in &self.steps {
            let share: f64 = Some(seconds / self.total_seconds * PERCENT)
                .filter(|share| share.is_finite())
                .unwrap_or_default();
            lines.push(format!(
                "  {name:<row_width$}  {seconds:>7.1}  {:>4.0}%",
                share
            ));
        }
        lines.push(format!(
            "  {:<row_width$}  {:>7.1}",
            "Total", self.total_seconds
        ));
        lines.join("\n")
    }

    /// The plan document's `timings`: total and steps in seconds, and what the plan read.
    pub fn json(&self, planned_from: &str) -> Value {
        let rounded = |seconds: f64| (seconds * MILLISECONDS).round() / MILLISECONDS;
        json!({
            "total_seconds": rounded(self.total_seconds),
            "steps": self
                .steps
                .iter()
                .map(|(name, seconds)| json!({"name": name, "seconds": rounded(*seconds)}))
                .collect::<Vec<Value>>(),
            "planned_from": planned_from,
        })
    }
}

/// The models and functions to rebuild: all with `--full-refresh`, else those of `full_refresh` jobs.
#[derive(Clone, Debug, Default)]
pub struct FullRefreshRequest {
    pub all: bool,
    pub models: HashSet<String>,
    pub functions: HashSet<String>,
}

impl FullRefreshRequest {
    /// The request of planner overrides' `full_refresh` and `full_refresh_*` names.
    pub fn of(overrides: &Bound<'_, PyAny>) -> PyResult<Self> {
        let names = |key: &str| -> PyResult<HashSet<String>> {
            native_field(overrides, key)?
                .try_iter()?
                .map(|name| name?.extract())
                .collect()
        };
        Ok(Self {
            all: native_field(overrides, FULL_REFRESH_KEY)?.is_truthy()?,
            models: names(FULL_REFRESH_MODELS_KEY)?,
            functions: names(FULL_REFRESH_FUNCTIONS_KEY)?,
        })
    }

    /// Whether the invocation asks to rebuild `model` from scratch.
    pub fn applies(&self, model: &str) -> bool {
        self.all || self.models.contains(model)
    }
}
