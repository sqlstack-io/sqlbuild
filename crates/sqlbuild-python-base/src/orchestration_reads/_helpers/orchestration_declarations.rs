//! The public authoring records enter native compilation here, never scheduler execution.

use crate::orchestration_reads::models::ImportedDeclarations;
use crate::orchestration_reads::models::RunControls;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyModuleMethods;
use pyo3::wrap_pyfunction;
use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyTuple};
use serde_json::Value;
use sqlbuild_discovery::orchestration_files::models::{self as files, RunFile};
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_orchestration::declarations::models::{DeclaredAt, OnFailure, Overlap, RunResources};

use crate::native_host::constants::AUTHORING_MODULE;
use crate::native_host::constants::CALENDAR_CLASS;
use crate::native_host::constants::CONSTRUCTING_MODULES;
use crate::native_host::constants::SENSOR_CLASS;
use crate::native_host::main::host_entry::host_entry as entry;

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

/// The controls of a TOML job or group; discovery has already checked the owner and the retry bounds.
pub(crate) fn run_controls(run: &RunFile) -> RunControls {
    RunControls {
        owner: run.owner.clone(),
        overlap: match run.overlap {
            files::Overlap::Queue => Overlap::Queue,
            files::Overlap::Skip => Overlap::Skip,
        },
        on_failure: run.on_failure.map(|policy| OnFailure {
            retries: policy.retries,
            retry_delay_seconds: policy.retry_delay_seconds,
        }),
    }
}

/// Refuse an unknown limit, a memory size Docker cannot read, and CPUs or a process limit that is not positive.
pub(crate) fn run_resources(
    name: &str,
    written: &serde_json::Map<String, Value>,
) -> PyResult<RunResources> {
    let invalid = |reason: String| PyValueError::new_err(format!("job {name} resources: {reason}"));
    let resources: RunResources = serde_json::from_value(Value::Object(written.clone()))
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
    Ok(resources)
}

/// Sort imported declarations into calendars and sensors, refusing a calendar name defined twice.
pub(crate) fn imported_declarations<'py>(
    py: Python<'py>,
    declarations: Vec<Bound<'py, PyAny>>,
) -> PyResult<ImportedDeclarations<'py>> {
    let sensor_class = py.import(AUTHORING_MODULE)?.getattr(SENSOR_CLASS)?;
    let mut imported = ImportedDeclarations {
        calendars: BTreeMap::new(),
        calendar_descriptions: BTreeMap::new(),
        sensors: Vec::new(),
    };
    for declaration in declarations {
        if declaration.is_instance(&sensor_class)? {
            imported.sensors.push(declaration.extract()?);
            continue;
        }
        let name: String = declaration.getattr("name")?.extract()?;
        let description: String = declaration.getattr("description")?.extract()?;
        if imported
            .calendars
            .insert(name.clone(), declaration)
            .is_some()
        {
            return Err(PyValueError::new_err(format!(
                "calendar '{name}' is declared twice"
            )));
        }
        imported.calendar_descriptions.insert(name, description);
    }
    Ok(imported)
}

/// The calendars and sensors one module defines; only module execution crosses into Python.
pub(crate) fn module_declarations<'py>(
    py: Python<'py>,
    file_path: &Path,
    project_dir: &Path,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let options = PyDict::new(py);
    options.set_item("file_path", file_path)?;
    options.set_item("project_dir", project_dir)?;
    let module = entry(py, UserCodeHost::LoadOrchestrationModule)?.call((), Some(&options))?;
    let authoring = py.import(AUTHORING_MODULE)?;
    let classes = [
        authoring.getattr(CALENDAR_CLASS)?,
        authoring.getattr(SENSOR_CLASS)?,
    ];
    let attributes = module.getattr("__dict__")?.cast_into::<PyDict>()?;
    let mut pending: Vec<Bound<'py, PyAny>> = attributes.values().iter().collect();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut found: Vec<Bound<'py, PyAny>> = Vec::new();
    while let Some(value) = pending.pop() {
        if !seen.insert(value.as_ptr() as usize) {
            continue;
        }
        if classes
            .iter()
            .any(|class| value.is_instance(class).unwrap_or(false))
        {
            found.push(value);
        } else if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
            pending.extend(value.try_iter()?.collect::<PyResult<Vec<_>>>()?);
        }
    }
    Ok(found)
}

/// Register `declaration_site` on the native module.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(declaration_site, module)?)
}
