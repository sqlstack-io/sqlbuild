//! The settings each planned node takes from the jobs its selection names, applied to the plan.

use std::collections::{BTreeMap, BTreeSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use serde_json::Value;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::selectors_need_jobs::selectors_need_jobs;
use sqlbuild_analysis::job_settings::constants::CONFLICT_HELP;
use sqlbuild_analysis::job_settings::errors::{JobSettingConflict, NodeJobSettingsError};
use sqlbuild_analysis::job_settings::main::build_job_settings::build_job_settings;
use sqlbuild_analysis::job_settings::main::named_jobs::named_jobs;
use sqlbuild_analysis::job_settings::main::node_job_settings::node_job_settings;
use sqlbuild_analysis::job_settings::models::{DeclaredRun, NodeJobSettings};

use crate::bindings::_helpers::compiled_project::job_settings::job_setting_conflict;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_project::compiled_project::main::graph_with_jobs::graph_with_jobs;
use sqlbuild_python_project::compiled_project::main::job_declarations::job_declarations;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::graph::models::KeyCache;
use sqlbuild_python_project::graph::models::NativeProjectGraph;

const MODEL_KIND: &str = "model";
const FULL_REFRESH: &str = "full_refresh";
const RUN_TESTS: &str = "run_tests";
const IGNORE_AUDIT_FAILURES: &str = "ignore_audit_failures";

/// What the planned build takes from its named jobs, or nothing when it names none.
pub(crate) struct PlannedJobSettings {
    settings: NodeJobSettings,
}

fn strings(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    value.try_iter()?.map(|item| item?.extract()).collect()
}

/// The named jobs' settings per selected node; `None` when ignored, unnamed or unselectable.
pub(crate) fn planned_job_settings(
    project: &Bound<'_, PyAny>,
    options: &Bound<'_, PyAny>,
) -> PyResult<Option<PlannedJobSettings>> {
    let py: Python<'_> = project.py();
    if !native_field(options, "job_settings")?.is_truthy()? {
        return Ok(None);
    }
    let select: Vec<String> = strings(&native_field(options, "select")?)?;
    let native: Bound<'_, NativeProjectGraph> =
        native_field(project, "lineage_graph")?.cast_into()?;
    let native = native.get();
    if !selectors_need_jobs(native.graph(), &select, &[]) {
        return Ok(None);
    }
    let exclude: Vec<String> = strings(&native_field(options, "exclude")?)?;
    let Some(declared): Option<BTreeMap<String, DeclaredRun>> = job_declarations(py)? else {
        return Ok(None);
    };
    let named: Vec<String> = named_jobs(&select, &declared);
    if named.is_empty() {
        return Ok(None);
    }
    build_job_settings(&named, &declared)
        .map_err(|conflict| job_setting_conflict(py, &conflict))?;
    let all: Vec<String> = select.iter().chain(&exclude).cloned().collect();
    let graph = graph_with_jobs(py, native.graph(), &all, &[])?;
    match node_job_settings(&graph, (&select, &exclude), &declared) {
        Ok(settings) => Ok(Some(PlannedJobSettings { settings })),
        Err(NodeJobSettingsError::Selector(_)) => Ok(None),
        Err(NodeJobSettingsError::Conflict(conflict)) => Err(job_setting_conflict(py, &conflict)),
    }
}

impl PlannedJobSettings {
    fn value(&self, key: &ObjectKey, setting: &str) -> Option<&Value> {
        self.settings
            .values
            .get(key)
            .and_then(|values| values.get(setting))
    }

    /// The models the named jobs rebuild from scratch.
    pub(crate) fn full_refresh_models<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyFrozenSet>> {
        let names: Vec<&String> = self
            .settings
            .values
            .iter()
            .filter(|(key, values)| {
                key.0 == MODEL_KIND && values.get(FULL_REFRESH) == Some(&Value::Bool(true))
            })
            .map(|(key, _)| &key.1)
            .collect();
        PyFrozenSet::new(py, names)
    }

    /// Each job's limit and each planned node's limited jobs; none under `--concurrency`.
    pub(crate) fn job_limits<'py>(
        &self,
        plan: &Bound<'py, PyAny>,
        cli_concurrency: &Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyDict>)> {
        let py: Python<'py> = plan.py();
        let limits: Bound<'py, PyDict> = PyDict::new(py);
        let node_jobs: Bound<'py, PyDict> = PyDict::new(py);
        if !cli_concurrency.is_none() || self.settings.job_limits.is_empty() {
            return Ok((limits, node_jobs));
        }
        for (job, limit) in &self.settings.job_limits {
            limits.set_item(job, limit)?;
        }
        let limited = |keys: &[ObjectKey]| -> Vec<String> {
            let mut jobs: Vec<String> = Vec::new();
            for job in keys
                .iter()
                .filter_map(|key| self.settings.node_jobs.get(key))
                .flatten()
                .filter(|job| self.settings.job_limits.contains_key(*job))
            {
                if !jobs.contains(job) {
                    jobs.push(job.clone());
                }
            }
            jobs
        };
        let cache = KeyCache::new(py)?;
        for key in self.settings.node_jobs.keys() {
            let jobs: Vec<String> = limited(std::slice::from_ref(key));
            if !jobs.is_empty() {
                node_jobs.set_item(cache.key(key)?, PyTuple::new(py, jobs)?)?;
            }
        }
        for entry in native_field(plan, "test_entries")?.try_iter()? {
            let entry = entry?;
            let mut keys: Vec<ObjectKey> = vec![native_key(&native_field(&entry, "key")?)?];
            for dep in native_field(&entry, "scope_deps")?.try_iter()? {
                keys.push(native_key(&dep?)?);
            }
            let jobs: Vec<String> = limited(&keys);
            if !jobs.is_empty() {
                node_jobs.set_item(native_field(&entry, "key")?, PyTuple::new(py, jobs)?)?;
            }
        }
        Ok((limits, node_jobs))
    }

    /// The plan with its jobs' `run_tests` and, unless the command line decided, audit failures.
    pub(crate) fn apply_to_plan<'py>(
        &self,
        plan: &Bound<'py, PyAny>,
        cli_ignore_audit_failures: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = plan.py();
        let mut skipped: BTreeSet<ObjectKey> = BTreeSet::new();
        let mut tests: Vec<Bound<'py, PyAny>> = Vec::new();
        for entry in native_field(plan, "test_entries")?.try_iter()? {
            let entry = entry?;
            let key: ObjectKey = native_key(&native_field(&entry, "key")?)?;
            if self.skips_test(&key, &native_field(&entry, "scope_deps")?)? {
                skipped.insert(key);
            } else {
                tests.push(entry);
            }
        }
        let mut changes: Vec<(&str, Bound<'py, PyAny>)> = Vec::new();
        if !skipped.is_empty() {
            let order: Vec<Bound<'py, PyAny>> = native_field(plan, "execution_order")?
                .try_iter()?
                .filter_map(|key| match key {
                    Ok(key) => match native_key(&key) {
                        Ok(native) if skipped.contains(&native) => None,
                        Ok(_) => Some(Ok(key)),
                        Err(error) => Some(Err(error)),
                    },
                    Err(error) => Some(Err(error)),
                })
                .collect::<PyResult<_>>()?;
            changes.push(("test_entries", PyTuple::new(py, tests)?.into_any()));
            changes.push(("execution_order", PyTuple::new(py, order)?.into_any()));
        }
        if cli_ignore_audit_failures.is_none() {
            let mut audits: Vec<Bound<'py, PyAny>> = Vec::new();
            let mut changed = false;
            for entry in native_field(plan, "audit_entries")?.try_iter()? {
                let entry = entry?;
                match self.audit_ignores(&entry)? {
                    Some(ignored)
                        if native_field(&entry, "ignore_failure")?.is_truthy()? != ignored =>
                    {
                        changed = true;
                        audits.push(replaced(
                            &entry,
                            &[(
                                "ignore_failure",
                                PyBool::new(py, ignored).to_owned().into_any(),
                            )],
                        )?);
                    }
                    _ => audits.push(entry),
                }
            }
            if changed {
                changes.push(("audit_entries", PyTuple::new(py, audits)?.into_any()));
            }
        }
        if changes.is_empty() {
            return Ok(plan.clone());
        }
        replaced(plan, &changes)
    }

    /// Whether a SQL test's models' jobs turn tests off; jobs that disagree conflict.
    fn skips_test(&self, test: &ObjectKey, scope_deps: &Bound<'_, PyAny>) -> PyResult<bool> {
        let mut keys: Vec<ObjectKey> = vec![test.clone()];
        for dep in scope_deps.try_iter()? {
            keys.push(native_key(&dep?)?);
        }
        let mut seen: Option<(&String, &Value)> = None;
        for key in &keys {
            let Some(value) = self.value(key, RUN_TESTS) else {
                continue;
            };
            let job: &String = self
                .settings
                .node_jobs
                .get(key)
                .and_then(|jobs| jobs.first())
                .unwrap_or(&key.1);
            match seen {
                Some((other, earlier)) if earlier != value => {
                    return Err(job_setting_conflict(
                        scope_deps.py(),
                        &JobSettingConflict {
                            message: format!(
                                "test {} covers nodes of jobs {other} and {job}, which set \
                                 run_tests differently ({earlier} and {value})",
                                test.1
                            ),
                            help: CONFLICT_HELP,
                        },
                    ));
                }
                Some(_) => {}
                None => seen = Some((job, value)),
            }
        }
        Ok(seen.is_some_and(|(_, value)| value == &Value::Bool(false)))
    }

    /// The `ignore_audit_failures` the audited node's job sets, if any.
    fn audit_ignores(&self, entry: &Bound<'_, PyAny>) -> PyResult<Option<bool>> {
        let kind: Bound<'_, PyAny> = native_field(entry, "attached_target_kind")?;
        let name: Bound<'_, PyAny> = native_field(entry, "attached_target_name")?;
        if kind.is_none() || name.is_none() {
            return Ok(None);
        }
        let kind: String = native_field(&kind, "value")?.extract()?;
        let key: ObjectKey = (kind, name.extract()?);
        Ok(self
            .value(&key, IGNORE_AUDIT_FAILURES)
            .and_then(Value::as_bool))
    }
}
