//! Load the project and local configuration files into their Python config objects.

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyOSError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBytes, PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple};
use sqlbuild_config::project::errors::{ConfigFailure, FailureClass, LoadFailure};
use sqlbuild_config::project::main::load_pool_limits::load_pool_limits;
use sqlbuild_config::project::models::PoolLimits;
use sqlbuild_config::project::models::{Built, ConfigFile, ConfigPaths};
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_python_base::boundary::main::config_value_to_python::config_value_to_python;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::registry::record_named::record_named;
use sqlbuild_python_base::records::types::ErrorId;
use std::path::PathBuf;

const FILE_NAMES: [&str; 4] = [
    PROJECT_CONFIG_FILENAME,
    "sqlbuild_project.yml",
    "sqlbuild_local.toml",
    "sqlbuild_local.yml",
];

/// The Python module of each `StrEnum` a loaded config holds that is not native.
const PYTHON_ENUM_MODULES: [(&str, &str); 1] =
    [("AuditSeverity", "sqlbuild.compiler.auditing.types")];

/// The four configuration file paths of a project directory, as Python and native paths.
pub(crate) struct Files<'py> {
    pub(crate) python: Vec<Bound<'py, PyAny>>,
    pub(crate) native: ConfigPaths,
}

pub(crate) fn files<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Files<'py>> {
    let python = FILE_NAMES
        .iter()
        .map(|name| project_dir.call_method1("__truediv__", (*name,)))
        .collect::<PyResult<Vec<_>>>()?;
    let native: Vec<PathBuf> = python
        .iter()
        .map(pyo3::types::PyAnyMethods::extract::<PathBuf>)
        .collect::<PyResult<_>>()?;
    let [project, legacy_project, local, legacy_local]: [PathBuf; 4] = native
        .try_into()
        .map_err(|_| PyOSError::new_err("configuration paths"))?;
    Ok(Files {
        python,
        native: ConfigPaths {
            project,
            legacy_project,
            local,
            legacy_local,
        },
    })
}

pub(crate) fn file_index(file: ConfigFile) -> usize {
    match file {
        ConfigFile::Project => 0,
        ConfigFile::LegacyProject => 1,
        ConfigFile::Local => 2,
        ConfigFile::LegacyLocal => 3,
    }
}

pub(crate) fn config_error(
    py: Python<'_>,
    failure: &ConfigFailure,
    files: &Files<'_>,
) -> PyResult<PyErr> {
    use sqlbuild_config::project::errors::MessagePiece;
    let pieces = failure
        .message
        .iter()
        .map(|piece| match piece {
            MessagePiece::Text(text) => Ok(PyString::new(py, text).into_any()),
            MessagePiece::File(file) => files.python[file_index(*file)].str().map(Bound::into_any),
        })
        .collect::<PyResult<Vec<_>>>()?;
    let message = PyString::new(py, "").call_method1("join", (pieces,))?;
    if failure.class == FailureClass::IntegerDigits {
        return Ok(PyValueError::new_err(message.unbind()));
    }
    let keywords = PyDict::new(py);
    keywords.set_item("help", failure.help.as_deref())?;
    let error = error_class(py, ErrorId::ProjectConfig)?.call((message,), Some(&keywords))?;
    Ok(PyErr::from_value(error))
}

fn os_error(py: Python<'_>, error: &std::io::Error, path: &Bound<'_, PyAny>) -> PyResult<PyErr> {
    let Some(errno) = error.raw_os_error() else {
        return Ok(PyOSError::new_err(error.to_string()));
    };
    let reason = py.import("os")?.call_method1("strerror", (errno,))?;
    let value = py
        .import("builtins")?
        .getattr("OSError")?
        .call1((errno, reason, path.str()?))?;
    Ok(PyErr::from_value(value))
}

pub(crate) fn load_error(
    py: Python<'_>,
    failure: &LoadFailure,
    files: &Files<'_>,
) -> PyResult<PyErr> {
    match failure {
        LoadFailure::Config(failure) => config_error(py, failure, files),
        LoadFailure::Io(file, error) => os_error(py, error, &files.python[file_index(*file)]),
        LoadFailure::Utf8(bytes) => Ok(PyBytes::new(py, bytes)
            .call_method0("decode")
            .err()
            .unwrap_or_else(|| PyOSError::new_err("invalid UTF-8"))),
    }
}

fn enum_member<'py>(py: Python<'py>, class: &str, value: &str) -> PyResult<Bound<'py, PyAny>> {
    let class_object: Bound<'py, PyAny> =
        match PYTHON_ENUM_MODULES.iter().find(|(name, _)| *name == class) {
            Some((_, module)) => py.import(*module)?.getattr(class)?,
            None => record_class(py, record_named(class)?)?,
        };
    class_object.call1((value,))
}

fn object<'py>(
    py: Python<'py>,
    class: &str,
    fields: Vec<(&'static str, Built)>,
) -> PyResult<Bound<'py, PyAny>> {
    let keywords = PyDict::new(py);
    for (name, value) in fields {
        keywords.set_item(name, built(py, value)?)?;
    }
    new_record(py, record_named(class)?, (), Some(&keywords))
}

pub(crate) fn built(py: Python<'_>, value: Built) -> PyResult<Py<PyAny>> {
    match value {
        Built::Value(value) => config_value_to_python(py, value),
        Built::Object(class, fields) => Ok(object(py, class, fields)?.unbind()),
        Built::Enum(class, member) => Ok(enum_member(py, class, &member)?.unbind()),
        Built::Dict(entries) => {
            let mapping = PyDict::new(py);
            for (key, item) in entries {
                mapping.set_item(key, built(py, item)?)?;
            }
            Ok(mapping.into_any().unbind())
        }
        Built::Tuple(items) => PyTuple::new(py, items)?.into_py_any(py),
        Built::FrozenSet(items) => PyFrozenSet::new(py, items)?.into_py_any(py),
        Built::Decimal(value) => {
            let number = config_value_to_python(py, value)?;
            let text = number.bind(py).str()?;
            Ok(py
                .import("decimal")?
                .getattr("Decimal")?
                .call1((text,))?
                .unbind())
        }
    }
}

/// One configuration section of the project at `project_dir`, with load failures raised as Python's.
pub(crate) fn configured<T>(
    project_dir: &Bound<'_, PyAny>,
    load: fn(&ConfigPaths) -> Result<T, LoadFailure>,
) -> PyResult<T> {
    let files: Files<'_> = files(project_dir)?;
    load(&files.native).or_else(|failure| Err(load_error(project_dir.py(), &failure, &files)?))
}

/// `[pools]` of `project_dir` and each target's pool limits.
pub(crate) fn project_pool_limits(
    py: Python<'_>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<PoolLimits> {
    let files = files(project_dir)?;
    load_pool_limits(&files.native).or_else(|failure| Err(load_error(py, &failure, &files)?))
}
