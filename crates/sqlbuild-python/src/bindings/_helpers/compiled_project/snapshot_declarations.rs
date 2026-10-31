//! Compile authoring records against retained graph data before publication.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pyo3::FromPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use serde_json::Value;
use sqlbuild_analysis::graph::main::resolve_selectors::resolve_selectors;
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_cli::argv::main::job_settings_arguments::job_settings_arguments;
use sqlbuild_core::constants::PYTHON_INIT_MODULE_STEM;
use sqlbuild_discovery::constants::{ORCHESTRATION_DIRECTORY, PYTHON_FILE_SUFFIX};
use sqlbuild_discovery::tree::main::rglob::rglob;
use sqlbuild_discovery::tree::models::ProjectTree;
use sqlbuild_orchestration::declarations::main::group_dependencies::group_dependencies;
use sqlbuild_orchestration::declarations::main::snapshot_definitions::snapshot_definitions;
use sqlbuild_orchestration::declarations::models::{
    GroupDefinition, JobDefinition, OnFailure, Schedule, TriggerDefinition,
};
use sqlbuild_orchestration::scheduler::models::Overlap;
use sqlbuild_orchestration::snapshots::models::SnapshotInput;

use crate::bindings::_helpers::native_host::orchestration_declarations::{
    authored_setting, module_jobs,
};

/// Dedicated declaration modules are imported only in the code-location interpreter.
pub(crate) fn discover_declarations<'py>(
    py: Python<'py>,
    project_dir: &Path,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let tree = ProjectTree::new(project_dir);
    let files = py
        .detach(|| {
            rglob(&tree, ORCHESTRATION_DIRECTORY, |entry| {
                !entry.is_dir
                    && entry.name.ends_with(PYTHON_FILE_SUFFIX)
                    && entry.name.strip_suffix(PYTHON_FILE_SUFFIX) != Some(PYTHON_INIT_MODULE_STEM)
            })
        })
        .map_err(|error| {
            PyValueError::new_err(format!("orchestration discovery failed: {error:?}"))
        })?;
    let mut declarations: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    for file in files {
        for job in module_jobs(py, &tree.absolute(&file), project_dir)? {
            if seen.insert(job.as_ptr() as usize) {
                declarations.push(job);
            }
        }
    }
    Ok(declarations)
}

#[derive(FromPyObject)]
struct AuthoredSchedule {
    cron: String,
    timezone: String,
    #[pyo3(from_py_with = authored_setting)]
    starts_at: Value,
    default_status: String,
    catch_up: String,
    #[pyo3(from_py_with = authored_setting)]
    max_catch_up: Value,
}

#[derive(FromPyObject)]
pub(crate) struct AuthoredJob {
    name: String,
    description: String,
    select: String,
    exclude: String,
    overlap: String,
    on_failure: String,
    #[pyo3(from_py_with = authored_setting)]
    settings: Value,
    #[pyo3(from_py_with = authored_setting)]
    concurrency: Value,
    concurrency_groups: Vec<String>,
    schedule: Option<AuthoredSchedule>,
    targets: Vec<String>,
}

#[derive(FromPyObject, PartialEq, Eq, PartialOrd, Ord)]
struct AuthoredJobReference {
    name: String,
}

#[derive(FromPyObject)]
pub(crate) struct AuthoredGroup {
    name: String,
    description: String,
    jobs: Vec<AuthoredJobReference>,
    after: BTreeMap<AuthoredJobReference, Vec<AuthoredJobReference>>,
    overlap: String,
    concurrency_groups: Vec<String>,
    schedule: Option<AuthoredSchedule>,
    targets: Vec<String>,
}

#[derive(FromPyObject)]
pub(crate) enum AuthoredDeclaration {
    Job(AuthoredJob),
    Group(AuthoredGroup),
}

impl AuthoredSchedule {
    fn compile(self, name: &str, description: &str) -> PyResult<TriggerDefinition> {
        let catch_up = if self.max_catch_up.is_null() {
            Value::String(self.catch_up)
        } else {
            serde_json::json!({self.catch_up: {"max_runs": self.max_catch_up}})
        };
        Ok(TriggerDefinition {
            description: description.into(),
            job_key: name.into(),
            schedule: Schedule::Cron { expression: self.cron, timezone: self.timezone },
            starts_at: serde_json::from_value(self.starts_at)
                .map_err(|error| PyValueError::new_err(format!("starts_at: {error}")))?,
            default_status: serde_json::from_value(Value::String(self.default_status))
                .map_err(|error| PyValueError::new_err(format!("default_status: {error}")))?,
            catch_up: serde_json::from_value(catch_up).map_err(|error| PyValueError::new_err(format!(
                "catch_up: {error}; use 'latest' without a bound or 'all' with a positive max_catch_up"
            )))?,
        })
    }
}

pub(crate) fn compile_declarations(
    mut input: SnapshotInput,
    graph: &ProjectGraph,
    declarations: Vec<AuthoredDeclaration>,
) -> PyResult<SnapshotInput> {
    if declarations.is_empty() {
        return Ok(input);
    }
    if ["jobs", "groups", "triggers"]
        .iter()
        .any(|key| input.documents.contains_key(*key))
    {
        return Err(PyValueError::new_err(
            "authored declarations cannot overwrite snapshot job or trigger documents",
        ));
    }
    let mut jobs: BTreeMap<String, JobDefinition> = BTreeMap::new();
    let mut authored_groups: Vec<AuthoredGroup> = Vec::new();
    let mut authored_jobs: Vec<AuthoredJob> = Vec::new();
    for declaration in declarations {
        match declaration {
            AuthoredDeclaration::Job(job) => authored_jobs.push(job),
            AuthoredDeclaration::Group(group) => authored_groups.push(group),
        }
    }
    let mut triggers: BTreeMap<String, TriggerDefinition> = BTreeMap::new();
    let mut names: BTreeSet<String> = BTreeSet::new();
    for job in authored_jobs {
        if !names.insert(job.name.clone()) {
            return Err(PyValueError::new_err(format!("duplicate job {}", job.name)));
        }
        if !job.targets.is_empty() && !job.targets.contains(&input.target) {
            continue;
        }
        let mut settings: BTreeMap<String, Value> = serde_json::from_value(job.settings)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        if !job.concurrency.is_null() {
            if settings.contains_key("concurrency") {
                return Err(PyValueError::new_err("job concurrency is declared twice"));
            }
            settings.insert("concurrency".into(), job.concurrency);
        }
        job_settings_arguments(&settings).map_err(PyValueError::new_err)?;
        let tokens = |value: &str| {
            value
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        if job.select.trim().is_empty() {
            return Err(PyValueError::new_err(format!(
                "job {} needs a selection",
                job.name
            )));
        }
        let selection: BTreeSet<String> =
            resolve_selectors(graph, &tokens(&job.select), &tokens(&job.exclude))
                .map_err(|error| PyValueError::new_err(error.message))?
                .into_iter()
                .map(|(kind, name)| format!("{kind}:{name}"))
                .collect();
        if let Some(schedule) = job.schedule {
            triggers.insert(
                job.name.clone(),
                schedule.compile(&job.name, &job.description)?,
            );
        }
        jobs.insert(
            job.name,
            JobDefinition {
                description: job.description,
                selection,
                settings,
                overlap: serde_json::from_value::<Overlap>(Value::String(job.overlap))
                    .map_err(|error| PyValueError::new_err(error.to_string()))?,
                on_failure: serde_json::from_value::<OnFailure>(Value::String(job.on_failure))
                    .map_err(|error| PyValueError::new_err(error.to_string()))?,
                concurrency_groups: job.concurrency_groups.into_iter().collect(),
            },
        );
    }
    let mut groups: BTreeMap<String, GroupDefinition> = BTreeMap::new();
    for group in authored_groups {
        if !names.insert(group.name.clone()) {
            return Err(PyValueError::new_err(format!(
                "duplicate job or group {}",
                group.name
            )));
        }
        if !group.targets.is_empty() && !group.targets.contains(&input.target) {
            continue;
        }
        let members: BTreeSet<String> = group.jobs.iter().map(|job| job.name.clone()).collect();
        if members.len() != group.jobs.len() {
            return Err(PyValueError::new_err("group members must be distinct"));
        }
        let mut after: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (job, dependencies) in group.after {
            after.insert(
                job.name,
                dependencies
                    .into_iter()
                    .map(|dependency| dependency.name)
                    .collect(),
            );
        }
        let dependencies = group_dependencies(&input, &jobs, (&members, &after))
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let selection: BTreeSet<String> = members
            .iter()
            .flat_map(|name| jobs[name].selection.iter().cloned())
            .collect();
        if let Some(schedule) = group.schedule {
            triggers.insert(
                group.name.clone(),
                schedule.compile(&group.name, &group.description)?,
            );
        }
        groups.insert(
            group.name,
            GroupDefinition {
                job: JobDefinition {
                    description: group.description,
                    selection,
                    settings: BTreeMap::new(),
                    overlap: serde_json::from_value(Value::String(group.overlap))
                        .map_err(|error| PyValueError::new_err(error.to_string()))?,
                    on_failure: OnFailure::SkipDependents,
                    concurrency_groups: group.concurrency_groups.into_iter().collect(),
                },
                jobs: dependencies,
            },
        );
    }
    for (key, document) in [
        ("jobs", serde_json::to_value(jobs)),
        ("groups", serde_json::to_value(groups)),
        ("triggers", serde_json::to_value(triggers)),
    ] {
        input.documents.insert(
            key.into(),
            document.map_err(|error| PyValueError::new_err(error.to_string()))?,
        );
    }
    snapshot_definitions(&input).map_err(|error| PyValueError::new_err(error.to_string()))?;
    Ok(input)
}
