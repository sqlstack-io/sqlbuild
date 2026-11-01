//! The public authoring records enter native compilation here, never scheduler execution.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use pyo3::FromPyObject;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyTuple};
use serde_json::Value;
use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::native_host::host::entry;

#[derive(FromPyObject)]
pub(crate) struct AuthoredJob {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) select: String,
    pub(crate) exclude: String,
    pub(crate) overlap: String,
    pub(crate) on_failure: String,
    #[pyo3(from_py_with = authored_setting)]
    pub(crate) settings: Value,
    #[pyo3(from_py_with = authored_setting)]
    pub(crate) concurrency: Value,
    pub(crate) concurrency_groups: Vec<String>,
    pub(crate) schedule: Option<Py<PyAny>>,
    pub(crate) targets: Vec<String>,
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
    pub(crate) overlap: String,
    pub(crate) concurrency_groups: Vec<String>,
    pub(crate) schedule: Option<Py<PyAny>>,
    pub(crate) targets: Vec<String>,
}

#[derive(FromPyObject)]
pub(crate) struct AuthoredPlatformCondition {
    pub(crate) kind: String,
    pub(crate) kinds: Vec<String>,
}

#[derive(FromPyObject)]
pub(crate) struct AuthoredSensor {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) when: AuthoredPlatformCondition,
    pub(crate) run: AuthoredJobReference,
    #[pyo3(from_py_with = authored_setting)]
    pub(crate) minimum_interval_seconds: Value,
    pub(crate) default_status: String,
    pub(crate) targets: Vec<String>,
}

pub(crate) enum AuthoredDeclaration {
    Job(AuthoredJob),
    Group(AuthoredGroup),
    Calendar(Py<PyAny>),
    Sensor(AuthoredSensor),
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
    let job_class = py.import("sqlbuild.orchestration")?.getattr("Job")?;
    let group_class = py.import("sqlbuild.orchestration")?.getattr("JobGroup")?;
    let calendar_class = py
        .import("sqlbuild.orchestration")?
        .getattr("CalendarSchedule")?;
    let sensor_class = py.import("sqlbuild.orchestration")?.getattr("Sensor")?;
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
    let job = py.import("sqlbuild.orchestration")?.getattr("Job")?;
    let group = py.import("sqlbuild.orchestration")?.getattr("JobGroup")?;
    let calendar = py
        .import("sqlbuild.orchestration")?
        .getattr("CalendarSchedule")?;
    let sensor = py.import("sqlbuild.orchestration")?.getattr("Sensor")?;
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
