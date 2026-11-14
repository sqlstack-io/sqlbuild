//! Compile TOML jobs and groups and Python calendars and sensors against retained graph data before publication.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use serde_json::Value;
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_cli::argv::main::job_settings_arguments::job_settings_arguments;
use sqlbuild_discovery::orchestration_files::models::{
    FilePosition, InitialStatus, OrchestrationInventory, RunFile, ScheduleSource,
};
use sqlbuild_discovery::tree::models::ProjectTree;
use sqlbuild_orchestration::declarations::main::group_dependencies::group_dependencies;
use sqlbuild_orchestration::declarations::main::snapshot_definitions::snapshot_definitions;
use sqlbuild_orchestration::declarations::models::{
    DeclaredAt, Definitions, GroupDefinition, JobDefinition, Schedule, TriggerDeclaration,
    TriggerDefinition, TriggerStatus,
};
use sqlbuild_orchestration::snapshots::models::SnapshotInput;

use crate::bindings::_helpers::graph::python_selection::ProjectSelection;
use crate::bindings::_helpers::native_host::constants::{AUTHORING_MODULE, SENSOR_CLASS};
use crate::bindings::_helpers::native_host::orchestration_declarations::{
    AuthoredSensor, module_declarations, run_controls, run_resources,
};
use crate::bindings::_helpers::native_host::orchestration_triggers::{
    CalendarRequest, compile_calendars, compile_sensors,
};
use sqlbuild_rules::dsl_headers::main::description_line_count::description_line_count;
use sqlbuild_rules::lint_text::main::resolve_format_settings::resolve_format_settings;
use sqlbuild_rules::lint_text::models::FormatSettings;

/// The calendars and sensors of the inventory's modules, imported by publication and by sensor evaluation.
pub(crate) fn python_declarations<'py>(
    py: Python<'py>,
    project_dir: &Path,
    inventory: &OrchestrationInventory,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let tree = ProjectTree::new(project_dir);
    let mut declarations: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    for module in inventory.calendars.iter().chain(&inventory.sensors) {
        for declaration in module_declarations(py, &tree.absolute(&module.path), project_dir)? {
            if seen.insert(declaration.as_ptr() as usize) {
                declarations.push(declaration);
            }
        }
    }
    Ok(declarations)
}

/// Where a TOML job or group is declared: its file and the line of its `name`.
fn file_site(path: &str, position: FilePosition) -> Option<DeclaredAt> {
    Some(DeclaredAt {
        path: path.to_owned(),
        line: u32::try_from(position.line).unwrap_or(u32::MAX),
    })
}

/// Whether a declaration with `targets` applies to `target`.
fn declared_for(targets: &[String], target: &str) -> bool {
    targets.is_empty() || targets.iter().any(|name| name == target)
}

/// A run's cron trigger, or the calendar it asks for, keyed by the run's name.
fn run_schedule(
    (path, run): (&str, &RunFile),
    triggers: &mut BTreeMap<String, TriggerDefinition>,
    calendars: &mut Vec<CalendarRequest>,
) -> PyResult<()> {
    let Some(schedule) = &run.schedule else {
        return Ok(());
    };
    let name: &str = &run.name.value;
    let starts_at = schedule
        .starts_at
        .as_ref()
        .map(|starts_at| serde_json::from_value(Value::String(starts_at.clone())))
        .transpose()
        .map_err(|error| PyValueError::new_err(format!("{path}: schedule starts_at: {error}")))?;
    let trigger = TriggerDefinition {
        description: run.description.clone(),
        job_key: name.to_owned(),
        schedule: Schedule::Cron {
            expression: String::new(),
            timezone: schedule.timezone.clone(),
        },
        starts_at,
        default_status: match schedule.default_status {
            InitialStatus::Running => TriggerStatus::Running,
            InitialStatus::Paused => TriggerStatus::Paused,
        },
        targets: run.targets.iter().cloned().collect(),
        declared_at: file_site(path, run.name.position),
    };
    match &schedule.source {
        ScheduleSource::Cron(expression) => {
            triggers.insert(
                name.to_owned(),
                TriggerDefinition {
                    schedule: Schedule::Cron {
                        expression: expression.clone(),
                        timezone: schedule.timezone.clone(),
                    },
                    ..trigger
                },
            );
        }
        ScheduleSource::Calendar(calendar) => calendars.push(CalendarRequest {
            calendar: calendar.value.clone(),
            timezone: schedule.timezone.clone(),
            trigger,
        }),
    }
    Ok(())
}

pub(crate) fn compile_declarations(
    py: Python<'_>,
    mut input: SnapshotInput,
    (project_dir, graph, selected): (&Path, &ProjectGraph, &ProjectSelection),
    (inventory, declarations): (&OrchestrationInventory, Vec<Bound<'_, PyAny>>),
) -> PyResult<SnapshotInput> {
    if inventory.is_empty() && declarations.is_empty() {
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
    let sensor_class = py.import(AUTHORING_MODULE)?.getattr(SENSOR_CLASS)?;
    let mut authored_calendars: BTreeMap<String, Bound<'_, PyAny>> = BTreeMap::new();
    let mut authored_sensors: Vec<AuthoredSensor> = Vec::new();
    for declaration in declarations {
        if declaration.is_instance(&sensor_class)? {
            authored_sensors.push(declaration.extract()?);
        } else {
            let name: String = declaration.getattr("name")?.extract()?;
            if authored_calendars
                .insert(name.clone(), declaration)
                .is_some()
            {
                return Err(PyValueError::new_err(format!(
                    "calendar '{name}' is declared twice"
                )));
            }
        }
    }
    let format: FormatSettings =
        resolve_format_settings(project_dir).map_err(PyValueError::new_err)?;
    let mut described: Vec<(&str, String, String)> = Vec::new();
    described.extend(inventory.jobs.iter().map(|(_, job)| {
        let run = &job.run;
        ("job", run.name.value.clone(), run.description.clone())
    }));
    described.extend(inventory.groups.iter().map(|(_, group)| {
        let run = &group.run;
        ("job group", run.name.value.clone(), run.description.clone())
    }));
    described.extend(
        authored_sensors
            .iter()
            .map(|sensor| ("sensor", sensor.name.clone(), sensor.description.clone())),
    );
    for (name, calendar) in &authored_calendars {
        described.push((
            "calendar",
            name.clone(),
            calendar.getattr("description")?.extract()?,
        ));
    }
    for (kind, name, description) in described {
        let limit: i64 = format.max_description_lines;
        let lines: usize = description_line_count(&description, format.line_width);
        if i64::try_from(lines).unwrap_or(i64::MAX) > limit {
            return Err(PyValueError::new_err(format!(
                "{kind} '{name}' description exceeds {limit} lines; shorten it to {limit} lines \
                 and move detail into documentation"
            )));
        }
    }
    let tokens = |value: &str| {
        value
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let mut jobs: BTreeMap<String, JobDefinition> = BTreeMap::new();
    let mut triggers: BTreeMap<String, TriggerDefinition> = BTreeMap::new();
    let mut calendar_requests: Vec<CalendarRequest> = Vec::new();
    for (path, job) in &inventory.jobs {
        let run = &job.run;
        if !declared_for(&run.targets, &input.target) {
            continue;
        }
        let name: &str = &run.name.value;
        job_settings_arguments(&job.settings)
            .map_err(|error| PyValueError::new_err(format!("job {name} settings: {error}")))?;
        let exclude: Option<String> = job
            .exclude
            .as_ref()
            .map(|exclude| exclude.value.clone())
            .filter(|exclude| !exclude.trim().is_empty());
        let selection: BTreeSet<String> = selected
            .run_keys(
                graph,
                &tokens(&job.select.value),
                &tokens(exclude.as_deref().unwrap_or_default()),
            )
            .map_err(|error| PyValueError::new_err(format!("job {name}: {error}")))?;
        let resources = job
            .resources
            .as_ref()
            .map(|resources| run_resources(name, &resources.value))
            .transpose()?;
        run_schedule((path, run), &mut triggers, &mut calendar_requests)?;
        let controls = run_controls(run);
        jobs.insert(
            name.to_owned(),
            JobDefinition {
                description: run.description.clone(),
                selection,
                settings: job.settings.clone(),
                concurrency_groups: run.concurrency_groups.iter().cloned().collect(),
                select: Some(job.select.value.clone()),
                exclude,
                targets: run.targets.iter().cloned().collect(),
                declared_at: file_site(path, run.name.position),
                owner: controls.owner,
                overlap: controls.overlap,
                on_failure: controls.on_failure,
                resources,
            },
        );
    }
    let mut groups: BTreeMap<String, GroupDefinition> = BTreeMap::new();
    for (path, group) in &inventory.groups {
        let run = &group.run;
        if !declared_for(&run.targets, &input.target) {
            continue;
        }
        let name: &str = &run.name.value;
        let members: BTreeSet<String> = group.jobs.iter().map(|job| job.value.clone()).collect();
        if members.len() != group.jobs.len() {
            return Err(PyValueError::new_err(format!(
                "job group {name} lists a job twice"
            )));
        }
        let dependencies = group_dependencies((name, &input), &jobs, &members)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let selection: BTreeSet<String> = members
            .iter()
            .flat_map(|member| jobs[member].selection.iter().cloned())
            .collect();
        run_schedule((path, run), &mut triggers, &mut calendar_requests)?;
        let controls = run_controls(run);
        groups.insert(
            name.to_owned(),
            GroupDefinition {
                job: JobDefinition {
                    description: run.description.clone(),
                    selection,
                    settings: BTreeMap::new(),
                    concurrency_groups: run.concurrency_groups.iter().cloned().collect(),
                    select: None,
                    exclude: None,
                    targets: run.targets.iter().cloned().collect(),
                    declared_at: file_site(path, run.name.position),
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
        (project_dir, &input, (graph, selected)),
        authored_sensors,
        compile_calendars(
            py,
            project_dir,
            (&authored_calendars, calendar_requests),
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
