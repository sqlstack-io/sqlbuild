//! The public authoring records enter native compilation here, never scheduler execution.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Component, Path};

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyTuple};
use pyo3::{FromPyObject, pyfunction};
use serde_json::Value;
use sqlbuild_analysis::graph::models::JobSelection;
use sqlbuild_discovery::project_validation::models::OrchestrationNameFact;
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_orchestration::declarations::models::{DeclaredAt, OnFailure, Overlap, RunResources};

use crate::bindings::_helpers::native_host::constants::{
    AUTHORING_MODULE, CALENDAR_LABEL, CONSTRUCTING_MODULES, GROUP_LABEL, JOB_LABEL, MAX_RETRIES,
    MAX_RETRY_DELAY, RUN_EVENT_CONDITION, SENSOR_LABEL,
};
use crate::bindings::_helpers::native_host::host::entry;

#[derive(FromPyObject)]
pub(crate) struct AuthoredJob {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) select: String,
    pub(crate) exclude: String,
    #[pyo3(from_py_with = authored_setting)]
    pub(crate) settings: Value,
    #[pyo3(from_py_with = authored_setting)]
    pub(crate) concurrency: Value,
    pub(crate) concurrency_groups: Vec<String>,
    pub(crate) schedule: Option<Py<PyAny>>,
    pub(crate) targets: Vec<String>,
    #[pyo3(attribute("_declared_at"))]
    pub(crate) declared_at: Option<(String, u32)>,
    pub(crate) owner: Option<String>,
    pub(crate) overlap: String,
    pub(crate) on_failure: Option<Py<PyAny>>,
    pub(crate) resources: Option<Py<PyAny>>,
}

#[derive(FromPyObject, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AuthoredJobReference {
    pub(crate) name: String,
}

#[derive(FromPyObject)]
pub(crate) struct AuthoredGroup {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) jobs: Vec<AuthoredJobReference>,
    pub(crate) after: BTreeMap<AuthoredJobReference, Vec<AuthoredJobReference>>,
    pub(crate) concurrency_groups: Vec<String>,
    pub(crate) schedule: Option<Py<PyAny>>,
    pub(crate) targets: Vec<String>,
    #[pyo3(attribute("_declared_at"))]
    pub(crate) declared_at: Option<(String, u32)>,
    pub(crate) owner: Option<String>,
    pub(crate) overlap: String,
    pub(crate) on_failure: Option<Py<PyAny>>,
}

/// A job's or group's owner, overlap and failure policy, validated where they enter native code.
pub(crate) struct RunControls {
    pub(crate) owner: Option<String>,
    pub(crate) overlap: Overlap,
    pub(crate) on_failure: Option<OnFailure>,
}

#[derive(FromPyObject)]
pub(crate) struct AuthoredSensorCondition {
    pub(crate) kind: String,
    #[pyo3(default)]
    pub(crate) kinds: Vec<String>,
    #[pyo3(default)]
    pub(crate) job: Option<AuthoredJobReference>,
    #[pyo3(default)]
    pub(crate) executed_successfully: Option<Vec<String>>,
    #[pyo3(default)]
    pub(crate) select: Option<String>,
    #[pyo3(default, from_py_with = authored_setting)]
    pub(crate) after_seconds: Value,
}

#[derive(FromPyObject)]
pub(crate) struct AuthoredSensor {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) when: Option<AuthoredSensorCondition>,
    pub(crate) function: Option<Py<PyAny>>,
    pub(crate) run: AuthoredJobReference,
    #[pyo3(from_py_with = authored_setting)]
    pub(crate) minimum_interval_seconds: Value,
    pub(crate) default_status: String,
    pub(crate) targets: Vec<String>,
    #[pyo3(attribute("_declared_at"))]
    pub(crate) declared_at: Option<(String, u32)>,
}

pub(crate) enum AuthoredDeclaration {
    Job(AuthoredJob),
    Group(AuthoredGroup),
    Calendar(Py<PyAny>),
    Sensor(AuthoredSensor),
}

/// The file and line of the author's code constructing a declaration; `None` for code without a source file.
#[pyfunction]
pub(crate) fn declaration_site(py: Python<'_>) -> PyResult<Option<(String, u32)>> {
    let mut frame = py.import("sys")?.call_method1("_getframe", (0,))?;
    while !frame.is_none() {
        let module: Option<String> = frame
            .getattr("f_globals")?
            .cast::<PyDict>()?
            .get_item("__name__")?
            .map(|name| name.extract())
            .transpose()?;
        if !module.is_some_and(|module| CONSTRUCTING_MODULES.contains(&module.as_str())) {
            let file: String = frame.getattr("f_code")?.getattr("co_filename")?.extract()?;
            let line: u32 = frame.getattr("f_lineno")?.extract()?;
            return Ok((!file.starts_with('<')).then_some((file, line)));
        }
        frame = frame.getattr("f_back")?;
    }
    Ok(None)
}

/// A declaration's file relative to the project; code outside the project keeps its location private.
pub(crate) fn declared_at(
    project_dir: &Path,
    written: Option<(String, u32)>,
) -> PyResult<Option<DeclaredAt>> {
    let Some((file, line)) = written else {
        return Ok(None);
    };
    let file = Path::new(&file);
    let roots = [
        project_dir.canonicalize()?,
        std::path::absolute(project_dir)?,
    ];
    let Some(root) = roots.iter().find(|root| file.starts_with(root)) else {
        return Ok(None);
    };
    let parts: Option<Vec<&str>> = file
        .components()
        .skip(root.components().count())
        .map(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    Ok(parts.map(|parts| DeclaredAt {
        path: parts.join("/"),
        line,
    }))
}

/// JSON authoring values cross once; the interpreter rejects circular user containers safely.
pub(crate) fn authored_setting(value: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    let text: String = value
        .py()
        .import("json")?
        .call_method1("dumps", (value,))?
        .extract()?;
    serde_json::from_str(&text).map_err(|error| PyValueError::new_err(error.to_string()))
}

/// Refuse a blank owner, an unknown overlap, and retries outside 1 to 10 or delays outside 0 to 24 hours.
pub(crate) fn run_controls(
    name: &str,
    (owner, overlap, policy): (Option<String>, &str, Option<&Bound<'_, PyAny>>),
) -> PyResult<RunControls> {
    if owner
        .as_deref()
        .is_some_and(|owner| owner.trim().is_empty())
    {
        return Err(PyValueError::new_err(format!(
            "job {name} owner must not be blank"
        )));
    }
    let overlap = match overlap {
        "queue" => Overlap::Queue,
        "skip" => Overlap::Skip,
        other => {
            return Err(PyValueError::new_err(format!(
                "job {name} overlap must be queue or skip, not {other}"
            )));
        }
    };
    let on_failure = if let Some(policy) = policy {
        let bounded = |field: &str, (low, high): (u64, u64)| -> PyResult<u64> {
            authored_setting(&policy.getattr(field)?)?
                .as_u64()
                .filter(|value| (low..=high).contains(value))
                .ok_or_else(|| {
                    PyValueError::new_err(format!(
                        "job {name} on_failure {field} must be a whole number from {low} to {high}"
                    ))
                })
        };
        Some(OnFailure {
            retries: u8::try_from(bounded("retries", MAX_RETRIES)?)?,
            retry_delay_seconds: u32::try_from(bounded("retry_delay_seconds", MAX_RETRY_DELAY)?)?,
        })
    } else {
        None
    };
    Ok(RunControls {
        owner,
        overlap,
        on_failure,
    })
}

/// Refuse a memory size Docker cannot read and CPUs or a process limit that is not positive.
pub(crate) fn job_resources(
    name: &str,
    resources: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<RunResources>> {
    let Some(resources) = resources else {
        return Ok(None);
    };
    let mut written = serde_json::Map::new();
    for field in ["memory", "cpus", "pids"] {
        let value = authored_setting(&resources.getattr(field)?)?;
        if !value.is_null() {
            written.insert(field.into(), value);
        }
    }
    let invalid = |reason: String| PyValueError::new_err(format!("job {name} resources: {reason}"));
    let resources: RunResources = serde_json::from_value(Value::Object(written))
        .map_err(|error| invalid(error.to_string()))?;
    if resources
        .cpus
        .is_some_and(|cpus| !cpus.is_normal() || cpus.is_sign_negative())
    {
        return Err(invalid("cpus must be a positive number".into()));
    }
    if resources.pids == Some(0) {
        return Err(invalid("pids must be a positive whole number".into()));
    }
    Ok(Some(resources))
}

/// Only module execution crosses into Python; discovery and collection stay native.
pub(crate) fn module_jobs<'py>(
    py: Python<'py>,
    file_path: &Path,
    project_dir: &Path,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let options = PyDict::new(py);
    options.set_item("file_path", file_path)?;
    options.set_item("project_dir", project_dir)?;
    let module = entry(py, UserCodeHost::LoadOrchestrationModule)?.call((), Some(&options))?;
    let job_class = py
        .import("sqlbuild.orchestration_declarations.models.authoring")?
        .getattr("Job")?;
    let group_class = py
        .import("sqlbuild.orchestration_declarations.models.authoring")?
        .getattr("JobGroup")?;
    let calendar_class = py
        .import("sqlbuild.orchestration_declarations.models.authoring")?
        .getattr("CalendarSchedule")?;
    let sensor_class = py
        .import("sqlbuild.orchestration_declarations.models.authoring")?
        .getattr("Sensor")?;
    let attributes = module.getattr("__dict__")?.cast_into::<PyDict>()?;
    let mut pending: Vec<Bound<'py, PyAny>> = attributes.values().iter().collect();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut jobs: Vec<Bound<'py, PyAny>> = Vec::new();
    while let Some(value) = pending.pop() {
        if !seen.insert(value.as_ptr() as usize) {
            continue;
        }
        if value.is_instance(&job_class)?
            || value.is_instance(&group_class)?
            || value.is_instance(&calendar_class)?
            || value.is_instance(&sensor_class)?
        {
            jobs.push(value);
        } else if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
            pending.extend(value.try_iter()?.collect::<PyResult<Vec<_>>>()?);
        }
    }
    Ok(jobs)
}

pub(crate) fn collect_declarations<'py>(
    py: Python<'py>,
    declarations: Vec<Bound<'py, PyAny>>,
) -> PyResult<Vec<AuthoredDeclaration>> {
    let job = py
        .import("sqlbuild.orchestration_declarations.models.authoring")?
        .getattr("Job")?;
    let group = py
        .import("sqlbuild.orchestration_declarations.models.authoring")?
        .getattr("JobGroup")?;
    let calendar = py
        .import("sqlbuild.orchestration_declarations.models.authoring")?
        .getattr("CalendarSchedule")?;
    let sensor = py
        .import("sqlbuild.orchestration_declarations.models.authoring")?
        .getattr("Sensor")?;
    let mut roots: HashSet<usize> = HashSet::new();
    for declaration in &declarations {
        if !roots.insert(declaration.as_ptr() as usize) {
            if declaration.is_instance(&calendar)? {
                return Err(PyValueError::new_err("duplicate calendar declaration"));
            }
            return Err(PyValueError::new_err(format!(
                "duplicate job {}",
                declaration.getattr("name")?.extract::<String>()?
            )));
        }
    }
    let mut pending = declarations;
    let mut seen: HashSet<usize> = HashSet::new();
    let mut collected: Vec<AuthoredDeclaration> = Vec::new();
    while let Some(declaration) = pending.pop() {
        if !seen.insert(declaration.as_ptr() as usize) {
            continue;
        }
        let authored = if declaration.is_instance(&sensor)? {
            let target = declaration.getattr("run")?;
            if !target.is_instance(&job)? && !target.is_instance(&group)? {
                return Err(PyTypeError::new_err(
                    "sensor run must be a Job or JobGroup object",
                ));
            }
            pending.push(target);
            let condition = declaration.getattr("when")?;
            if !condition.is_none()
                && condition.getattr("kind")?.extract::<String>()? == RUN_EVENT_CONDITION
            {
                let source = condition.getattr("job")?;
                if !source.is_instance(&job)? && !source.is_instance(&group)? {
                    return Err(PyTypeError::new_err(
                        "run_event job must be a Job or JobGroup object",
                    ));
                }
                pending.push(source);
            }
            AuthoredDeclaration::Sensor(declaration.extract()?)
        } else if declaration.is_instance(&calendar)? {
            let target = declaration.getattr("job")?;
            if !target.is_instance(&job)? && !target.is_instance(&group)? {
                return Err(PyTypeError::new_err(
                    "calendar job must be a Job or JobGroup object",
                ));
            }
            pending.push(target);
            AuthoredDeclaration::Calendar(declaration.unbind())
        } else if declaration.is_instance(&group)? {
            for member in declaration.getattr("jobs")?.try_iter()? {
                let member = member?;
                if !member.is_instance(&job)? {
                    return Err(PyTypeError::new_err("group members must be Job objects"));
                }
                pending.push(member);
            }
            for (member, dependencies) in declaration.getattr("after")?.cast::<PyDict>()?.iter() {
                if !member.is_instance(&job)? {
                    return Err(PyTypeError::new_err(
                        "group dependencies must be Job objects",
                    ));
                }
                for dependency in dependencies.try_iter()? {
                    if !dependency?.is_instance(&job)? {
                        return Err(PyTypeError::new_err(
                            "group dependencies must be Job objects",
                        ));
                    }
                }
            }
            AuthoredDeclaration::Group(declaration.extract()?)
        } else if !declaration.is_instance(&job)? {
            return Err(PyTypeError::new_err(
                "snapshot declarations must be Job, JobGroup, CalendarSchedule or Sensor objects",
            ));
        } else {
            AuthoredDeclaration::Job(declaration.extract()?)
        };
        collected.push(authored);
    }
    Ok(collected)
}

/// Every declaration reachable from `roots`, once each, with its kind and the module of the root that reaches it first.
pub(crate) fn reachable_declarations<'py>(
    py: Python<'py>,
    roots: &[(Bound<'py, PyAny>, String)],
) -> PyResult<Vec<(Bound<'py, PyAny>, &'static str, String)>> {
    let authoring = py.import(AUTHORING_MODULE)?;
    let classes = [
        (authoring.getattr("Job")?, JOB_LABEL),
        (authoring.getattr("JobGroup")?, GROUP_LABEL),
        (authoring.getattr("Sensor")?, SENSOR_LABEL),
        (authoring.getattr("CalendarSchedule")?, CALENDAR_LABEL),
    ];
    let mut seen: HashSet<usize> = HashSet::new();
    let mut reached: Vec<(Bound<'py, PyAny>, &'static str, String)> = Vec::new();
    for (root, path) in roots {
        let mut pending: Vec<Bound<'py, PyAny>> = vec![root.clone()];
        while let Some(declaration) = pending.pop() {
            if !seen.insert(declaration.as_ptr() as usize) {
                continue;
            }
            let Some(kind) = classes
                .iter()
                .find(|(class, _)| declaration.is_instance(class).unwrap_or(false))
                .map(|(_, kind)| *kind)
            else {
                continue;
            };
            pending.extend(nested_declarations(&declaration, kind)?);
            reached.push((declaration, kind, path.clone()));
        }
    }
    Ok(reached)
}

/// Every named declaration reachable from `roots`, labelled with the module of the root that reaches it first.
pub(crate) fn orchestration_names(
    py: Python<'_>,
    roots: &[(Bound<'_, PyAny>, String)],
) -> PyResult<Vec<OrchestrationNameFact>> {
    let mut names: Vec<OrchestrationNameFact> = Vec::new();
    for (declaration, kind, path) in reachable_declarations(py, roots)? {
        let name: Option<String> = if kind == CALENDAR_LABEL {
            let function = declaration.getattr("function")?;
            (!function.is_none())
                .then(|| function.getattr("__name__")?.extract())
                .transpose()?
        } else {
            Some(declaration.getattr("name")?.extract()?)
        };
        names.extend(name.map(|name| OrchestrationNameFact { name, kind, path }));
    }
    Ok(names)
}

/// The declarations one declaration refers to: a group's jobs, a calendar's job, a sensor's job and run-event job.
fn nested_declarations<'py>(
    declaration: &Bound<'py, PyAny>,
    kind: &str,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    Ok(match kind {
        GROUP_LABEL => declaration
            .getattr("jobs")?
            .try_iter()?
            .collect::<PyResult<Vec<_>>>()?,
        CALENDAR_LABEL => vec![declaration.getattr("job")?],
        SENSOR_LABEL => {
            let condition = declaration.getattr("when")?;
            let mut nested = vec![declaration.getattr("run")?];
            if !condition.is_none()
                && condition.getattr("kind")?.extract::<String>()? == RUN_EVENT_CONDITION
            {
                nested.push(condition.getattr("job")?);
            }
            nested
        }
        _ => Vec::new(),
    })
}

/// Each reachable job's own selector tokens and each group's member names for `target`, as publication reads them.
pub(crate) fn job_selections(
    py: Python<'_>,
    roots: &[(Bound<'_, PyAny>, String)],
    target: Option<&str>,
) -> PyResult<BTreeMap<String, JobSelection>> {
    let mut catalog: BTreeMap<String, JobSelection> = BTreeMap::new();
    for (declaration, kind, _) in reachable_declarations(py, roots)? {
        if kind != JOB_LABEL && kind != GROUP_LABEL {
            continue;
        }
        if !declared_for(&declaration, target)? {
            continue;
        }
        let tokens = |field: &str| -> PyResult<Vec<String>> {
            Ok(declaration
                .getattr(field)?
                .extract::<String>()?
                .split_whitespace()
                .map(str::to_owned)
                .collect())
        };
        let selection: JobSelection = match kind {
            JOB_LABEL => JobSelection::Job {
                select: tokens("select")?,
                exclude: tokens("exclude")?,
            },
            GROUP_LABEL => JobSelection::Group {
                members: declaration
                    .getattr("jobs")?
                    .try_iter()?
                    .map(|job| job?.getattr("name")?.extract())
                    .collect::<PyResult<_>>()?,
            },
            _ => continue,
        };
        let name: String = declaration.getattr("name")?.extract()?;
        if catalog.insert(name.clone(), selection).is_some() {
            return Err(PyValueError::new_err(format!(
                "job name '{name}' is declared twice"
            )));
        }
    }
    Ok(catalog)
}

/// Whether a declaration applies to `target`: it names no targets, or names this one.
fn declared_for(declaration: &Bound<'_, PyAny>, target: Option<&str>) -> PyResult<bool> {
    let targets: Vec<String> = declaration.getattr("targets")?.extract()?;
    Ok(
        targets.is_empty()
            || target.is_some_and(|target| targets.iter().any(|name| name == target)),
    )
}

/// The jobs and groups that a schedule, calendar schedule or sensor declared for `target` starts.
pub(crate) fn triggered_job_names(
    py: Python<'_>,
    roots: &[(Bound<'_, PyAny>, String)],
    target: Option<&str>,
) -> PyResult<BTreeSet<String>> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for (declaration, kind, _) in reachable_declarations(py, roots)? {
        if !declared_for(&declaration, target)? {
            continue;
        }
        let started: Option<Bound<'_, PyAny>> = match kind {
            JOB_LABEL | GROUP_LABEL => {
                (!declaration.getattr("schedule")?.is_none()).then(|| declaration.clone())
            }
            CALENDAR_LABEL => Some(declaration.getattr("job")?),
            SENSOR_LABEL => Some(declaration.getattr("run")?),
            _ => None,
        };
        if let Some(job) = started {
            names.insert(job.getattr("name")?.extract()?);
        }
    }
    Ok(names)
}
