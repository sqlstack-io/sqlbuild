//! The settings each planned node takes from the jobs its selection names, applied to the plan.

use std::collections::{BTreeMap, BTreeSet};

use pyo3::IntoPyObject;
use pyo3::PyErr;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use serde_json::Value;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::definition_changes::constants::PYTHON_NODE_KINDS;
use sqlbuild_analysis::graph::main::selectors_need_jobs::selectors_need_jobs;
use sqlbuild_analysis::job_settings::constants::WAREHOUSE_SETTING;
use sqlbuild_analysis::job_settings::errors::{JobSettingConflict, NodeJobSettingsError};
use sqlbuild_analysis::job_settings::main::conflict_help::conflict_help;
use sqlbuild_analysis::job_settings::main::consumed_setting::consumed_setting;
use sqlbuild_analysis::job_settings::main::resolve_job_settings::resolve_job_settings;
use sqlbuild_analysis::job_settings::models::{
    DeclaredRun, NodeConsumer, NodeJobSettings, ResolvedJobSettings,
};
use sqlbuild_config::command_connection::errors::CommandConnectionError;
use sqlbuild_config::command_connection::main::check_job_warehouse::check_job_warehouse;
use sqlbuild_config::command_connection::main::unsupported_job_warehouse::unsupported_job_warehouse;
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_project::job_settings::main::given_settings::given_settings;

use crate::compile_driver::main::setup::project_graph::project_graph;
use crate::compile_driver::models::PlannedJobSettings;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_front::compile_driver::main::session_warehouse_support::session_warehouse_support;
use sqlbuild_python_front::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use sqlbuild_python_project::compiled_project::constants::{
    JOB_FULL_REFRESH_KEY, JOB_IGNORED_AUDIT_FAILURES_KEY, JOB_WAREHOUSES_KEY,
};
use sqlbuild_python_project::compiled_views::main::project::audits::audits;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::graph::main::python_nodes::python_node_graph::python_node_graph;
use sqlbuild_python_project::graph::models::KeyCache;
use sqlbuild_python_project::graph::models::{
    CompiledProjectGraph, NativeProjectGraph, PythonAwareSelection,
};
use sqlbuild_python_project::job_settings::main::job_declarations::job_declarations;
use sqlbuild_python_project::job_settings::main::job_scope_project_dir::job_scope_project_dir;
use sqlbuild_python_project::job_settings::main::job_selector_error::job_selector_error;
use sqlbuild_python_project::job_settings::main::job_setting_conflict::job_setting_conflict;
use sqlbuild_python_project::resource_views::main::audit_reads::attached_target_kind::attached_target_kind;
use sqlbuild_python_project::resource_views::main::audit_reads::attached_target_name::attached_target_name;
use sqlbuild_python_project::resource_views::main::audit_reads::key::key as audit_key;
use sqlbuild_python_project::resource_views::main::audit_reads::scope_deps::scope_deps;

const MODEL_KIND: &str = "model";
const FUNCTION_KINDS: [&str; 2] = ["udf", "table_fn"];
/// The node setting `consumer` applies, as the job setting table names it.
pub(crate) fn node_setting(consumer: NodeConsumer) -> &'static str {
    consumed_setting(consumer).unwrap_or_default()
}

/// The `CliUserError` of a job warehouse that cannot be used.
fn cli_error(py: Python<'_>, error: &CommandConnectionError) -> PyErr {
    let (code, help) = match error {
        CommandConnectionError::User { code, help, .. } => (*code, help.as_deref()),
        CommandConnectionError::ProjectConfig(_) => (None, None),
    };
    new_error(py, ErrorId::CliUser, error.message(), (code, help))
}

fn is_python(key: &ObjectKey) -> bool {
    PYTHON_NODE_KINDS.contains(&key.0.as_str())
}

/// The jobs whose `warehouse` a node runs on, checked like `--warehouse` (`C261`, `C260`).
fn job_warehouses(
    (project, discovered): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    settings: &NodeJobSettings,
    declared: &BTreeMap<String, DeclaredRun>,
) -> PyResult<BTreeMap<String, String>> {
    let py: Python<'_> = project.py();
    let mut warehouses: BTreeMap<String, String> = BTreeMap::new();
    for (key, jobs) in &settings.node_jobs {
        if !settings
            .values
            .get(key)
            .is_some_and(|values| values.contains_key(node_setting(NodeConsumer::NodeWarehouses)))
        {
            continue;
        }
        for job in jobs {
            if warehouses.contains_key(job) {
                continue;
            }
            if let Some(DeclaredRun::Job { settings, .. }) = declared.get(job)
                && let Some(value) = settings.get(node_setting(NodeConsumer::NodeWarehouses))
            {
                let name: String = check_job_warehouse(job, value.as_str())
                    .map_err(|error| cli_error(py, &error))?;
                warehouses.insert(job.clone(), name);
            }
        }
    }
    let Some((job, warehouse)) = warehouses.iter().next() else {
        return Ok(warehouses);
    };
    let adapter_name: Bound<'_, PyAny> = resolve_effective_adapter_name(
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
    )?;
    let project_dir: Bound<'_, PyAny> = job_scope_project_dir().into_pyobject(py)?.into_any();
    if !session_warehouse_support(py, &adapter_name, &project_dir)? {
        let adapter: String = adapter_name.str()?.extract()?;
        return Err(cli_error(
            py,
            &unsupported_job_warehouse(job, warehouse, &adapter),
        ));
    }
    Ok(warehouses)
}

/// The named jobs of `settings` that set `setting` to true for a selected node of `kind`, if given.
fn jobs_setting(
    settings: &NodeJobSettings,
    declared: &BTreeMap<String, DeclaredRun>,
    (setting, kinds): (&str, Option<&[&str]>),
) -> Vec<String> {
    let mut jobs: BTreeSet<String> = BTreeSet::new();
    for (key, node_jobs) in &settings.node_jobs {
        if is_python(key) || kinds.is_some_and(|kinds| !kinds.contains(&key.0.as_str())) {
            continue;
        }
        for job in node_jobs {
            if let Some(DeclaredRun::Job { settings, .. }) = declared.get(job)
                && settings.get(setting) == Some(&Value::Bool(true))
            {
                jobs.insert(job.clone());
            }
        }
    }
    jobs.into_iter().collect()
}

/// Whether every read node ignores, by job value or else `build_ignores`; `None` without job values.
pub(crate) fn singular_audit_ignores(
    job_values: impl Iterator<Item = Option<bool>>,
    build_ignores: bool,
) -> Option<bool> {
    let values: Vec<Option<bool>> = job_values.collect();
    values
        .iter()
        .any(Option::is_some)
        .then(|| values.iter().all(|value| value.unwrap_or(build_ignores)))
}

/// The nodes each audit attached to no node reads, by the audit's key.
fn unattached_audit_reads(
    project: &Bound<'_, PyAny>,
) -> PyResult<BTreeMap<ObjectKey, Vec<ObjectKey>>> {
    let mut reads: BTreeMap<ObjectKey, Vec<ObjectKey>> = BTreeMap::new();
    for audit in audits(project)?.try_iter()? {
        let audit: Bound<'_, PyAny> = audit?;
        if !attached_target_kind(&audit)?.is_none() && !attached_target_name(&audit)?.is_none() {
            continue;
        }
        let deps: Vec<ObjectKey> = scope_deps(&audit)?
            .try_iter()?
            .map(|dep| native_key(&dep?))
            .collect::<PyResult<_>>()?;
        reads.insert(native_key(&audit_key(&audit)?)?, deps);
    }
    Ok(reads)
}

fn strings(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    value.try_iter()?.map(|item| item?.extract()).collect()
}

/// The named jobs' settings per selected node; `None` when ignored, unnamed or unselectable.
pub(crate) fn planned_job_settings(
    (project, discovered): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (options, cli_warehouse): (&Bound<'_, PyAny>, bool),
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
    let mut given: BTreeSet<String> = given_settings();
    if cli_warehouse {
        given.insert(WAREHOUSE_SETTING.to_owned());
    }
    let all: Vec<String> = select.iter().chain(&exclude).cloned().collect();
    let compiled: Bound<'_, CompiledProjectGraph> = project_graph(project)?.cast_into()?;
    let selection = PythonAwareSelection::new(
        (&compiled, native.graph()),
        &python_node_graph(discovered)?,
        &all,
    )?;
    let token_nodes =
        |token: &str, exclude: &[String]| selection.resolve_keys(&[token.to_owned()], exclude);
    let resolved = compiler_guard(|| {
        Ok(resolve_job_settings(
            declared,
            (&select, &given),
            Some((selection.graph(), (&select, &exclude), &token_nodes)),
        ))
    })?;
    match resolved {
        Ok(ResolvedJobSettings {
            nodes: Some(settings),
            declared,
            ..
        }) => Ok(Some(PlannedJobSettings {
            ignoring_jobs: jobs_setting(
                &settings,
                &declared,
                (node_setting(NodeConsumer::AuditEntries), None),
            ),
            refreshing_jobs: jobs_setting(
                &settings,
                &declared,
                (
                    node_setting(NodeConsumer::RefreshOverrides),
                    Some(&[MODEL_KIND, FUNCTION_KINDS[0], FUNCTION_KINDS[1]]),
                ),
            ),
            job_warehouses: job_warehouses((project, discovered), &settings, &declared)?,
            unattached_audit_reads: unattached_audit_reads(project)?,
            settings,
        })),
        Ok(ResolvedJobSettings { nodes: None, .. }) => Ok(None),
        Err(NodeJobSettingsError::Selector(failure)) => Err(job_selector_error(py, &failure)),
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

    /// The names of the nodes of `kinds` the named jobs rebuild from scratch.
    fn refreshed<'py>(&self, py: Python<'py>, kinds: &[&str]) -> PyResult<Bound<'py, PyFrozenSet>> {
        let names: Vec<&String> = self
            .settings
            .values
            .iter()
            .filter(|(key, values)| {
                kinds.contains(&key.0.as_str())
                    && values.get(node_setting(NodeConsumer::RefreshOverrides))
                        == Some(&Value::Bool(true))
            })
            .map(|(key, _)| &key.1)
            .collect();
        PyFrozenSet::new(py, names)
    }

    /// The models the named jobs rebuild from scratch.
    pub fn full_refresh_models<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyFrozenSet>> {
        self.refreshed(py, &[MODEL_KIND])
    }

    /// The functions the named jobs recreate.
    pub fn full_refresh_functions<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyFrozenSet>> {
        self.refreshed(py, &FUNCTION_KINDS)
    }

    /// Each job's limit and each planned node's limited jobs; none under `--concurrency`.
    pub(crate) fn job_limits<'py>(
        &self,
        plan: &Bound<'py, PyAny>,
        cli_concurrency: bool,
    ) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyDict>)> {
        let py: Python<'py> = plan.py();
        let limits: Bound<'py, PyDict> = PyDict::new(py);
        let node_jobs: Bound<'py, PyDict> = PyDict::new(py);
        if cli_concurrency || self.settings.job_limits.is_empty() {
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
        for key in self.settings.node_jobs.keys().filter(|key| !is_python(key)) {
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

    /// The warehouse of a node's named job, when one applies to it.
    fn warehouse(&self, key: &ObjectKey) -> Option<&str> {
        if self.job_warehouses.is_empty() {
            return None;
        }
        self.value(key, node_setting(NodeConsumer::NodeWarehouses))
            .and_then(Value::as_str)
    }

    /// Each SQL node's job warehouse, a test's only if all its nodes share it, and Python nodes'.
    pub(crate) fn node_warehouses<'py>(
        &self,
        plan: &Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyDict>)> {
        let py: Python<'py> = plan.py();
        let nodes: Bound<'py, PyDict> = PyDict::new(py);
        let python: Bound<'py, PyDict> = PyDict::new(py);
        if self.job_warehouses.is_empty() {
            return Ok((nodes, python));
        }
        for key in native_field(plan, "execution_order")?.try_iter()? {
            let key: Bound<'py, PyAny> = key?;
            if let Some(warehouse) = self.warehouse(&native_key(&key)?) {
                nodes.set_item(key, warehouse)?;
            }
        }
        for entry in native_field(plan, "test_entries")?.try_iter()? {
            let entry: Bound<'py, PyAny> = entry?;
            let mut covered: BTreeSet<Option<&str>> = BTreeSet::new();
            for dep in native_field(&entry, "scope_deps")?.try_iter()? {
                covered.insert(self.warehouse(&native_key(&dep?)?));
            }
            let key: Bound<'py, PyAny> = native_field(&entry, "key")?;
            match covered.into_iter().collect::<Vec<_>>().as_slice() {
                [Some(warehouse)] => nodes.set_item(key, *warehouse)?,
                _ if nodes.contains(&key)? => nodes.del_item(key)?,
                _ => {}
            }
        }
        for key in self.settings.values.keys().filter(|key| is_python(key)) {
            if let Some(warehouse) = self.warehouse(key) {
                python.set_item(&key.1, warehouse)?;
            }
        }
        Ok((nodes, python))
    }

    /// The plan with its jobs' `run_tests` and, unless the command line decided, audit failures.
    pub(crate) fn apply_to_plan<'py>(
        &self,
        plan: &Bound<'py, PyAny>,
        (cli_ignore_audit_failures, build_ignores): (&Bound<'py, PyAny>, bool),
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
                match self.audit_ignores(&entry, build_ignores)? {
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
        let ignoring: &[String] = if cli_ignore_audit_failures.is_none() {
            &self.ignoring_jobs
        } else {
            &[]
        };
        if !ignoring.is_empty()
            || !self.refreshing_jobs.is_empty()
            || !self.job_warehouses.is_empty()
        {
            let metadata: Bound<'py, PyDict> = native_field(plan, "metadata")?
                .cast_into::<PyDict>()?
                .copy()?;
            if !ignoring.is_empty() {
                metadata.set_item(JOB_IGNORED_AUDIT_FAILURES_KEY, PyTuple::new(py, ignoring)?)?;
            }
            if !self.refreshing_jobs.is_empty() {
                metadata.set_item(
                    JOB_FULL_REFRESH_KEY,
                    PyTuple::new(py, &self.refreshing_jobs)?,
                )?;
            }
            if !self.job_warehouses.is_empty() {
                let warehouses: Bound<'py, PyDict> = PyDict::new(py);
                for (job, warehouse) in &self.job_warehouses {
                    warehouses.set_item(job, warehouse)?;
                }
                metadata.set_item(JOB_WAREHOUSES_KEY, warehouses)?;
            }
            changes.push(("metadata", metadata.into_any()));
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
            let Some(value) = self.value(key, node_setting(NodeConsumer::TestEntries)) else {
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
                            help: conflict_help(node_setting(NodeConsumer::TestEntries)),
                        },
                    ));
                }
                Some(_) => {}
                None => seen = Some((job, value)),
            }
        }
        Ok(seen.is_some_and(|(_, value)| value == &Value::Bool(false)))
    }

    /// The audited node's `ignore_audit_failures`, or for a singular audit, its read nodes' together.
    fn audit_ignores(
        &self,
        entry: &Bound<'_, PyAny>,
        build_ignores: bool,
    ) -> PyResult<Option<bool>> {
        let kind: Bound<'_, PyAny> = native_field(entry, "attached_target_kind")?;
        let name: Bound<'_, PyAny> = native_field(entry, "attached_target_name")?;
        if kind.is_none() || name.is_none() {
            let audit: ObjectKey = native_key(&native_field(entry, "key")?)?;
            let reads: &[ObjectKey] = self
                .unattached_audit_reads
                .get(&audit)
                .map_or(&[], Vec::as_slice);
            return Ok(singular_audit_ignores(
                reads.iter().map(|key| {
                    self.value(key, node_setting(NodeConsumer::AuditEntries))
                        .and_then(Value::as_bool)
                }),
                build_ignores,
            ));
        }
        let kind: String = native_field(&kind, "value")?.extract()?;
        let key: ObjectKey = (kind, name.extract()?);
        Ok(self
            .value(&key, node_setting(NodeConsumer::AuditEntries))
            .and_then(Value::as_bool))
    }
}
