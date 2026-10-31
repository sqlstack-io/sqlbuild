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
use sqlbuild_orchestration::declarations::main::snapshot_definitions::snapshot_definitions;
use sqlbuild_orchestration::declarations::models::{
    JobDefinition, OnFailure, Schedule, TriggerDefinition, TriggerStatus,
};
use sqlbuild_orchestration::scheduler::models::{CatchUp, Overlap};
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

pub(crate) fn compile_declarations(
    mut input: SnapshotInput,
    graph: &ProjectGraph,
    declarations: Vec<AuthoredJob>,
) -> PyResult<SnapshotInput> {
    if declarations.is_empty() {
        return Ok(input);
    }
    if input.documents.contains_key("jobs") || input.documents.contains_key("triggers") {
        return Err(PyValueError::new_err(
            "authored declarations cannot overwrite snapshot job or trigger documents",
        ));
    }
    let mut jobs: BTreeMap<String, JobDefinition> = BTreeMap::new();
    let mut triggers: BTreeMap<String, TriggerDefinition> = BTreeMap::new();
    let mut names: BTreeSet<String> = BTreeSet::new();
    for job in declarations {
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
                TriggerDefinition {
                    description: job.description.clone(),
                    job_key: job.name.clone(),
                    schedule: Schedule::Cron {
                        expression: schedule.cron,
                        timezone: schedule.timezone,
                    },
                    starts_at: None,
                    default_status: TriggerStatus::Running,
                    catch_up: CatchUp::Latest,
                },
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
    for (key, document) in [
        ("jobs", serde_json::to_value(jobs)),
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
