//! Load the project and local configuration files into their Python config objects.

use std::path::PathBuf;

use pyo3::exceptions::{PyOSError, PyValueError};
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBytes, PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_config::project::errors::{ConfigFailure, FailureClass, LoadFailure};
use sqlbuild_config::project::main::deprecated_setting_warnings::deprecated_setting_warnings;
use sqlbuild_config::project::main::load_git_base_ref::load_git_base_ref;
use sqlbuild_config::project::main::load_local_config::load_local_config;
use sqlbuild_config::project::main::load_project_config::load_project_config;
use sqlbuild_config::project::main::load_review_config::load_review_config;
use sqlbuild_config::project::main::local_sql_analysis_failure::local_sql_analysis_failure;
use sqlbuild_config::project::main::resolve_config_file::resolve_config_file;
use sqlbuild_config::project::models::{Built, ConfigFile, ConfigPaths, ReviewConfig};

use crate::bindings::_helpers::boundary::config_values::config_value_to_python;
use crate::bindings::_helpers::native_host::host::write_console;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::{new_record, record_class};
use crate::bindings::_helpers::records::ids::ErrorId;
use crate::bindings::_helpers::records::registry;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;

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
struct Files<'py> {
    python: Vec<Bound<'py, PyAny>>,
    native: ConfigPaths,
}

fn files<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Files<'py>> {
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

fn file_index(file: ConfigFile) -> usize {
    match file {
        ConfigFile::Project => 0,
        ConfigFile::LegacyProject => 1,
        ConfigFile::Local => 2,
        ConfigFile::LegacyLocal => 3,
    }
}

fn config_error(py: Python<'_>, failure: &ConfigFailure, files: &Files<'_>) -> PyResult<PyErr> {
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

fn load_error(py: Python<'_>, failure: &LoadFailure, files: &Files<'_>) -> PyResult<PyErr> {
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
            None => record_class(py, registry::record_named(class)?)?,
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
    new_record(py, registry::record_named(class)?, (), Some(&keywords))
}

fn built(py: Python<'_>, value: Built) -> PyResult<Py<PyAny>> {
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

/// Write a warning line to stderr for each deprecated setting alias `project_dir` still sets.
pub(crate) fn warn_deprecated_settings(
    py: Python<'_>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let files = files(project_dir)?;
    match deprecated_setting_warnings(&files.native) {
        Ok(warnings) => warnings
            .iter()
            .try_for_each(|warning| write_console(py, "stderr", &format!("{warning}\n"))),
        Err(failure) => Err(load_error(py, &failure, &files)?),
    }
}

/// `load_project_config`: the validated `ProjectConfig` of `project_dir`.
#[pyfunction]
pub(crate) fn load_project_config_file(
    py: Python<'_>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let files = files(project_dir)?;
    match load_project_config(&files.native) {
        Ok(config) => built(py, config),
        Err(failure) => Err(load_error(py, &failure, &files)?),
    }
}

/// `[git] base_ref` of the project at `project_dir`, which `--since @base` names.
pub(crate) fn configured_git_base_ref(project_dir: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let files = files(project_dir)?;
    load_git_base_ref(&files.native)
        .or_else(|failure| Err(load_error(project_dir.py(), &failure, &files)?))
}

/// `[review]` of the project at `project_dir`: the defaults `sqb review` falls back to.
pub(crate) fn configured_review(project_dir: &Bound<'_, PyAny>) -> PyResult<ReviewConfig> {
    let files = files(project_dir)?;
    load_review_config(&files.native)
        .or_else(|failure| Err(load_error(project_dir.py(), &failure, &files)?))
}

/// `load_local_config`: the validated `LocalConfig` of `project_dir`, or the default.
#[pyfunction]
pub(crate) fn load_local_config_file(
    py: Python<'_>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let files = files(project_dir)?;
    match load_local_config(&files.native) {
        Ok(config) => built(py, config),
        Err(failure) => Err(load_error(py, &failure, &files)?),
    }
}

/// The project or local configuration file path, rejecting an unconverted legacy file.
#[pyfunction]
pub(crate) fn resolve_config_file_path<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
    local: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let files = files(project_dir)?;
    match resolve_config_file(&files.native, local) {
        Ok(file) => Ok(files.python[file_index(file)].clone()),
        Err(failure) => Err(config_error(py, &failure, &files)?),
    }
}

/// `validate_local_sql_analysis_policy` from the project requirement and the local setting.
#[pyfunction]
pub(crate) fn validate_local_sql_analysis(
    py: Python<'_>,
    project_dir: &Bound<'_, PyAny>,
    flags: (bool, bool, bool),
) -> Result<(), PyErr> {
    let files = files(project_dir)?;
    match local_sql_analysis_failure(&files.native, flags) {
        None => Ok(()),
        Some(failure) => Err(config_error(py, &failure, &files)?),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(load_project_config_file, module)?)?;
    module.add_function(wrap_pyfunction!(load_local_config_file, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_config_file_path, module)?)?;
    module.add_function(wrap_pyfunction!(validate_local_sql_analysis, module)?)
}
