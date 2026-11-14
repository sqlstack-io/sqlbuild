//! Compile authoring records against retained graph data before publication.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
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
    DeclaredAt, Definitions, GroupDefinition, JobDefinition, TriggerDeclaration, TriggerDefinition,
};
use sqlbuild_orchestration::snapshots::models::SnapshotInput;

use crate::bindings::_helpers::native_host::orchestration_declarations::{
    AuthoredDeclaration, AuthoredGroup, AuthoredJob, AuthoredSensor, declared_at, job_resources,
    module_jobs, run_controls,
};
use crate::bindings::_helpers::native_host::orchestration_triggers::{
    compile_calendars, compile_cron, compile_sensors,
};

/// Dedicated declaration modules, imported by publication and by selectors that name a job.
pub(crate) fn discover_declarations<'py>(
    py: Python<'py>,
    project_dir: &Path,
) -> PyResult<Vec<(Bound<'py, PyAny>, String)>> {
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
    let mut declarations: Vec<(Bound<'py, PyAny>, String)> = Vec::new();
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    for file in files {
        for job in module_jobs(py, &tree.absolute(&file), project_dir)? {
            if seen.insert(job.as_ptr() as usize) {
                declarations.push((job, file.clone()));
            }
        }
    }
    Ok(declarations)
}

pub(crate) fn compile_declarations(
    py: Python<'_>,
    mut input: SnapshotInput,
    (project_dir, graph): (&Path, &ProjectGraph),
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
    let mut authored_calendars: Vec<Py<PyAny>> = Vec::new();
    let mut authored_sensors: Vec<AuthoredSensor> = Vec::new();
    for declaration in declarations {
        match declaration {
            AuthoredDeclaration::Job(job) => authored_jobs.push(job),
            AuthoredDeclaration::Group(group) => authored_groups.push(group),
            AuthoredDeclaration::Calendar(calendar) => authored_calendars.push(calendar),
            AuthoredDeclaration::Sensor(sensor) => authored_sensors.push(sensor),
        }
    }
    let mut triggers: BTreeMap<String, TriggerDefinition> = BTreeMap::new();
    for job in authored_jobs {
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
        let declared_at: Option<DeclaredAt> = declared_at(project_dir, job.declared_at)?;
        let controls = run_controls(
            &job.name,
            (
                job.owner,
                &job.overlap,
                job.on_failure.as_ref().map(|policy| policy.bind(py)),
            ),
        )?;
        let resources = job_resources(
            &job.name,
            job.resources.as_ref().map(|resources| resources.bind(py)),
        )?;
        let targets: BTreeSet<String> = job.targets.into_iter().collect();
        if let Some(schedule) = job.schedule {
            triggers.insert(
                job.name.clone(),
                TriggerDefinition {
                    targets: targets.clone(),
                    declared_at: declared_at.clone(),
                    ..compile_cron(schedule.bind(py), &job.name, &job.description)?
                },
            );
        }
        jobs.insert(
            job.name,
            JobDefinition {
                description: job.description,
                selection,
                settings,
                concurrency_groups: job.concurrency_groups.into_iter().collect(),
                select: Some(job.select),
                exclude: Some(job.exclude).filter(|exclude| !exclude.trim().is_empty()),
                targets,
                declared_at,
                owner: controls.owner,
                overlap: controls.overlap,
                on_failure: controls.on_failure,
                resources,
            },
        );
    }
    let mut groups: BTreeMap<String, GroupDefinition> = BTreeMap::new();
    for group in authored_groups {
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
        let dependencies = group_dependencies((&group.name, &input), &jobs, (&members, &after))
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let selection: BTreeSet<String> = members
            .iter()
            .flat_map(|name| jobs[name].selection.iter().cloned())
            .collect();
        let declared_at: Option<DeclaredAt> = declared_at(project_dir, group.declared_at)?;
        let controls = run_controls(
            &group.name,
            (
                group.owner,
                &group.overlap,
                group.on_failure.as_ref().map(|policy| policy.bind(py)),
            ),
        )?;
        let targets: BTreeSet<String> = group.targets.into_iter().collect();
        if let Some(schedule) = group.schedule {
            triggers.insert(
                group.name.clone(),
                TriggerDefinition {
                    targets: targets.clone(),
                    declared_at: declared_at.clone(),
                    ..compile_cron(schedule.bind(py), &group.name, &group.description)?
                },
            );
        }
        groups.insert(
            group.name,
            GroupDefinition {
                job: JobDefinition {
                    description: group.description,
                    selection,
                    settings: BTreeMap::new(),
                    concurrency_groups: group.concurrency_groups.into_iter().collect(),
                    select: None,
                    exclude: None,
                    targets,
                    declared_at,
                    owner: controls.owner,
                    overlap: controls.overlap,
                    on_failure: controls.on_failure,
                    resources: None,
                },
                jobs: dependencies,
            },
        );
    }
    let Definitions {
        jobs,
        groups,
        triggers,
        sensors,
        ..
    } = compile_sensors(
        py,
        (project_dir, &input, graph),
        authored_sensors,
        compile_calendars(
            py,
            (project_dir, &input.target),
            authored_calendars,
            Definitions {
                jobs,
                groups,
                triggers,
                ..Definitions::default()
            },
        )?,
    )?;
    let triggers: BTreeMap<_, _> = triggers
        .into_iter()
        .map(|(name, trigger)| (name, TriggerDeclaration::Schedule(trigger)))
        .chain(
            sensors
                .into_iter()
                .map(|(name, sensor)| (name, TriggerDeclaration::Sensor(sensor))),
        )
        .collect();
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
