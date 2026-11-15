//! Project-tree queries custom rules run over the discovered paths.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::bindings::_helpers::rules::fact_tree::{Raised, VALUE_ERROR, strerror, text_error};
use pyo3::exceptions::PyOSError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyString, PyStringMethods, PyTuple};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_rules::project_tree::errors::{
    ProjectTextError, ProjectTreeFileError, RelativePartsError,
};
use sqlbuild_rules::project_tree::main::project_relative_parts::project_relative_parts;
use sqlbuild_rules::project_tree::main::project_tree_children::project_tree_children;
use sqlbuild_rules::project_tree::main::project_tree_descendants::project_tree_descendants;
use sqlbuild_rules::project_tree::main::project_tree_glob::project_tree_glob;
use sqlbuild_rules::project_tree::main::read_project_text::read_project_text;
use sqlbuild_rules::project_tree::main::resolve_project_path::resolve_project_path;

use crate::bindings::_helpers::rules::rule_api::{usage_error_for, validate_project_relative};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

const PATHLIB_MODULE: &str = "pathlib";

/// `pathlib.Path(value)`, raising exactly as `Path` does for an unsupported value.
fn python_path<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    imported(value.py(), PATHLIB_MODULE, "Path")?.call1((value,))
}

/// The nodes and their `value`s as raw paths.
fn node_values<'py>(nodes: &Bound<'py, PyAny>) -> PyResult<(Vec<Bound<'py, PyAny>>, Vec<PathBuf>)> {
    let nodes: Vec<Bound<'py, PyAny>> = nodes.try_iter()?.collect::<PyResult<_>>()?;
    let values: Vec<PathBuf> = nodes
        .iter()
        .map(|node| native_field(node, "value")?.extract())
        .collect::<PyResult<_>>()?;
    Ok((nodes, values))
}

/// The nodes at `indexes`, in order.
fn selected<'py>(
    py: Python<'py>,
    nodes: &[Bound<'py, PyAny>],
    indexes: Vec<usize>,
) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, indexes.into_iter().map(|index| nodes[index].clone()))
}

/// The nodes directly inside the project directory `path`.
#[pyfunction]
fn project_tree_child_nodes<'py>(
    nodes: &Bound<'py, PyAny>,
    path: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let directory: PathBuf = python_path(path)?.extract()?;
    let (nodes, values) = node_values(nodes)?;
    let values: Vec<&std::path::Path> = values.iter().map(PathBuf::as_path).collect();
    selected(
        path.py(),
        &nodes,
        project_tree_children(&values, &directory),
    )
}

/// The nodes below the project directory `path`, or every node for an empty path.
#[pyfunction]
fn project_tree_descendant_nodes<'py>(
    nodes: &Bound<'py, PyAny>,
    path: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let directory: Option<PathBuf> = if path.is_truthy()? {
        Some(python_path(path)?.extract()?)
    } else {
        None
    };
    let (nodes, values) = node_values(nodes)?;
    let values: Vec<&std::path::Path> = values.iter().map(PathBuf::as_path).collect();
    selected(
        path.py(),
        &nodes,
        project_tree_descendants(&values, directory.as_deref()),
    )
}

/// The nodes a project-relative shell-style pattern matches.
#[pyfunction]
fn project_tree_glob_nodes<'py>(
    nodes: &Bound<'py, PyAny>,
    pattern: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = pattern.py();
    validate_project_relative(pattern)?;
    let pattern: PathBuf = pattern.extract()?;
    let (nodes, values) = node_values(nodes)?;
    let values: Vec<&std::path::Path> = values.iter().map(PathBuf::as_path).collect();
    selected(py, &nodes, project_tree_glob(&values, &pattern))
}

/// `RuleUsageError(f"project path {path} is not beneath {under}")`, spelled by Python.
fn not_beneath(path: &Bound<'_, PyAny>, under: &Bound<'_, PyAny>) -> PyErr {
    let py: Python<'_> = path.py();
    let message = path.str().and_then(|path_text| {
        let under_text = under.str()?;
        PyString::new(py, "project path ")
            .as_any()
            .add(path_text)?
            .add(" is not beneath ")?
            .add(under_text)
    });
    match message.and_then(|message| {
        error_class(py, ErrorId::RuleUsage).and_then(|class| class.call1((message,)))
    }) {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// `path`'s parts relative to the resolved project `root`, then to `under` when given.
#[pyfunction]
#[pyo3(signature = (*, root, path, under))]
fn project_tree_relative_parts<'py>(
    root: &Bound<'py, PyAny>,
    path: &Bound<'py, PyAny>,
    under: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = path.py();
    let relative: Bound<'py, PyAny> = python_path(path)?;
    let under_path: Option<PathBuf> = if under.is_truthy()? {
        Some(python_path(under)?.extract()?)
    } else {
        None
    };
    let parts: Vec<OsString> = match project_relative_parts(
        &relative.extract::<PathBuf>()?,
        &root.extract::<PathBuf>()?,
        under_path.as_deref(),
        resolve_project_path,
    ) {
        Ok(parts) => parts,
        Err(RelativePartsError::Resolve(_) | RelativePartsError::Escapes) => {
            return Err(usage_error_for("project path escapes the project: ", path));
        }
        Err(RelativePartsError::NotBeneath) => return Err(not_beneath(path, under)),
    };
    let parts: Vec<Bound<'py, PyAny>> = parts
        .into_iter()
        .map(|part| Ok(part.into_pyobject(py)?.into_any()))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, parts)
}

/// The UTF-8 text of the tracked project file at `path` under the resolved `root`.
#[pyfunction]
fn project_tree_read_text(root: PathBuf, path: &Bound<'_, PyAny>) -> PyResult<String> {
    let py: Python<'_> = path.py();
    let text: String = path.str()?.to_string_lossy().into_owned();
    let raised: Raised = match read_project_text(&root, Path::new(&text)) {
        Ok(contents) => return Ok(contents),
        Err(ProjectTextError::File(ProjectTreeFileError::Unreadable { path, errno })) => {
            return Err(unreadable(py, path, errno));
        }
        #[cfg(not(unix))]
        Err(ProjectTextError::Read { path: file, .. }) => {
            return Err(python_read_error(py, &file, &text));
        }
        Err(error) => text_error(&error, &text),
    };
    if raised.kind == VALUE_ERROR {
        return Err(PyValueError::new_err(raised.message));
    }
    Err(PyErr::from_value(
        error_class(py, ErrorId::RuleUsage)?.call1((raised.message,))?,
    ))
}

/// The `OSError` `Path.is_file()` raises for a stat failure it does not ignore.
#[cfg(unix)]
fn unreadable(py: Python<'_>, path: PathBuf, errno: i32) -> PyErr {
    let Ok(path) = path.into_os_string().into_pyobject(py);
    PyOSError::new_err((errno, strerror(errno), path.unbind()))
}

/// The `OSError` `Path.is_file()` raises for a Windows stat failure it does not ignore.
#[cfg(not(unix))]
fn unreadable(py: Python<'_>, path: PathBuf, code: i32) -> PyErr {
    let detail: String = strerror(code);
    let detail: &str =
        detail.trim_end_matches(|character: char| character <= ' ' || character == '.');
    let Ok(path) = path.into_os_string().into_pyobject(py);
    PyOSError::new_err((code, detail.to_owned(), path.unbind(), code))
}

/// The usage error for a Windows read failure, worded by Python's own `Path.read_text`.
#[cfg(not(unix))]
fn python_read_error(py: Python<'_>, file: &Path, text: &str) -> PyErr {
    let read = || -> PyResult<PyErr> {
        let reading = imported(py, "pathlib", "Path")?
            .call1((file.as_os_str(),))?
            .call_method1("read_text", ("utf-8",));
        let error: PyErr = match reading {
            Ok(_) => {
                return Ok(PyOSError::new_err(format!(
                    "could not read project file {text}"
                )));
            }
            Err(error) => error,
        };
        let message: String = format!(
            "could not read project file {text}: {}",
            error.value(py).str()?
        );
        let usage: PyErr =
            PyErr::from_value(error_class(py, ErrorId::RuleUsage)?.call1((message,))?);
        usage.set_cause(py, Some(error));
        Ok(usage)
    };
    read().unwrap_or_else(|error| error)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(project_tree_child_nodes, module)?)?;
    module.add_function(wrap_pyfunction!(project_tree_descendant_nodes, module)?)?;
    module.add_function(wrap_pyfunction!(project_tree_glob_nodes, module)?)?;
    module.add_function(wrap_pyfunction!(project_tree_relative_parts, module)?)?;
    module.add_function(wrap_pyfunction!(project_tree_read_text, module)?)?;
    Ok(())
}
