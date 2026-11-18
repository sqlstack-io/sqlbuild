//! The command target's jobs, loaded when a selector names one or `--changed` needs triggered jobs.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use pyo3::IntoPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::graph::main::parse_selector::parse_selector;
use sqlbuild_analysis::graph::models::{JobSelection, ParsedSelector};
use sqlbuild_analysis::job_settings::main::declared_runs::declared_runs;
use sqlbuild_analysis::job_settings::models::DeclaredRun;
use sqlbuild_cli::native_commands::main::effective_target::effective_target;
use sqlbuild_discovery::orchestration_files::models::{OrchestrationInventory, RunFile};

use crate::compiled_project::constants::JOB_SELECTOR_KINDS;
use crate::compiled_project::constants::SELECTOR_INTERSECTION;
use crate::compiled_project::constants::SENSOR_RUN_ARGUMENT;
use sqlbuild_python_front::discovery::main::sources::orchestration_declarations::orchestration_declarations;

/// Each job and group's selection, and the names of those a schedule or sensor starts.
type Catalog = (BTreeMap<String, JobSelection>, BTreeSet<String>);

/// The running command's project directory, its `--target` and, once loaded, its jobs and their settings.
struct JobScope {
    project_dir: PathBuf,
    selected_target: Option<String>,
    catalog: Option<(Catalog, BTreeMap<String, DeclaredRun>)>,
}

thread_local! {
    /// Per thread, since a command resolves its selectors on the thread that dispatches it.
    static JOB_SCOPE: RefCell<Option<JobScope>> = const { RefCell::new(None) };
}

/// Run one command with jobs selectable from `project_dir`, restoring the enclosing scope after.
pub(crate) fn with_job_scope<T>(
    (project_dir, selected_target): (PathBuf, Option<String>),
    run: impl FnOnce() -> T,
) -> T {
    let previous = JOB_SCOPE.with(|scope| {
        scope.replace(Some(JobScope {
            project_dir,
            selected_target,
            catalog: None,
        }))
    });
    let result: T = run();
    JOB_SCOPE.with(|scope| *scope.borrow_mut() = previous);
    result
}

/// The jobs `selectors` name, as `job:<name>` or as a bare job name.
pub(crate) fn selected_jobs(
    selectors: &[String],
    catalog: &BTreeMap<String, JobSelection>,
) -> BTreeSet<String> {
    ordered_jobs(selectors, catalog).into_iter().collect()
}

/// The jobs `selectors` name, each once in the order first named.
pub(crate) fn ordered_jobs(
    selectors: &[String],
    catalog: &BTreeMap<String, JobSelection>,
) -> Vec<String> {
    let mut jobs: Vec<String> = Vec::new();
    let parts = selectors
        .iter()
        .flat_map(|selector| selector.split(char::is_whitespace))
        .flat_map(|token| token.split(SELECTOR_INTERSECTION));
    for part in parts {
        let Ok(ParsedSelector::Kind { kind, value, .. }) = parse_selector(part) else {
            continue;
        };
        if JOB_SELECTOR_KINDS.contains(&kind.as_str())
            && catalog.contains_key(&value)
            && !jobs.contains(&value)
        {
            jobs.push(value);
        }
    }
    jobs
}

pub(crate) fn job_catalog(py: Python<'_>) -> PyResult<Option<Catalog>> {
    Ok(loaded_jobs(py)?.map(|(catalog, _)| catalog))
}

/// Each job's `[settings]` and `[retry]` and each group's jobs declared for the command's target; none outside a command.
pub(crate) fn job_declarations(py: Python<'_>) -> PyResult<Option<BTreeMap<String, DeclaredRun>>> {
    Ok(loaded_jobs(py)?.map(|(_, declared)| declared))
}

fn loaded_jobs(py: Python<'_>) -> PyResult<Option<(Catalog, BTreeMap<String, DeclaredRun>)>> {
    let Some(scope) = JOB_SCOPE.with(|scope| {
        scope.borrow().as_ref().map(|scope| match &scope.catalog {
            Some(loaded) => Ok(loaded.clone()),
            None => Err((scope.project_dir.clone(), scope.selected_target.clone())),
        })
    }) else {
        return Ok(None);
    };
    let (project_dir, selected_target): (PathBuf, Option<String>) = match scope {
        Ok(loaded) => return Ok(Some(loaded)),
        Err(scope) => scope,
    };
    let target: Option<String> =
        effective_target(&project_dir, selected_target).map_err(PyValueError::new_err)?;
    let inventory: OrchestrationInventory =
        orchestration_declarations(project_dir.clone().into_pyobject(py)?.as_any())?;
    let loaded = (
        inventory_catalog(&inventory, target.as_deref()),
        declared_runs(&inventory, target.as_deref()),
    );
    JOB_SCOPE.with(|scope| {
        if let Some(scope) = scope.borrow_mut().as_mut() {
            scope.catalog = Some(loaded.clone());
        }
    });
    Ok(Some(loaded))
}

/// Each job's selector tokens and each group's members declared for `target`, and the names a schedule or sensor starts there.
fn inventory_catalog(inventory: &OrchestrationInventory, target: Option<&str>) -> Catalog {
    catalog_where(inventory, |targets: &[String]| {
        targets.is_empty() || target.is_some_and(|target| targets.iter().any(|name| name == target))
    })
}

/// Each job's selector tokens and each group's members `declared_for` accepts the targets of.
pub(crate) fn catalog_where(
    inventory: &OrchestrationInventory,
    declared_for: impl Fn(&[String]) -> bool,
) -> Catalog {
    let tokens = |value: &str| value.split_whitespace().map(str::to_owned).collect();
    let mut catalog: BTreeMap<String, JobSelection> = BTreeMap::new();
    let mut triggered: BTreeSet<String> = BTreeSet::new();
    let mut add = |run: &RunFile, selection: JobSelection| {
        if declared_for(&run.targets) {
            if run.schedule.is_some() {
                triggered.insert(run.name.value.clone());
            }
            catalog.insert(run.name.value.clone(), selection);
        }
    };
    for (_, job) in &inventory.jobs {
        add(
            &job.run,
            JobSelection::Job {
                select: tokens(&job.select.value),
                exclude: job
                    .exclude
                    .as_ref()
                    .map(|exclude| tokens(&exclude.value))
                    .unwrap_or_default(),
            },
        );
    }
    let job_targets: BTreeMap<&str, &[String]> = inventory
        .jobs
        .iter()
        .map(|(_, job)| (job.run.name.value.as_str(), job.run.targets.as_slice()))
        .collect();
    for (_, group) in &inventory.groups {
        add(
            &group.run,
            JobSelection::Group {
                members: group
                    .jobs
                    .iter()
                    .filter(|job| {
                        job_targets
                            .get(job.value.as_str())
                            .is_none_or(|targets| declared_for(targets))
                    })
                    .map(|job| job.value.clone())
                    .collect(),
            },
        );
    }
    for sensor in &inventory.sensors {
        if sensor.targets.as_deref().is_none_or(&declared_for) {
            triggered.extend(
                sensor
                    .job_references
                    .iter()
                    .filter(|(argument, _)| *argument == SENSOR_RUN_ARGUMENT)
                    .map(|(_, job)| job.value.clone()),
            );
        }
    }
    (catalog, triggered)
}
