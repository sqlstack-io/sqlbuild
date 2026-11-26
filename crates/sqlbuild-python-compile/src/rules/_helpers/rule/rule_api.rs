//! Custom-rule API decisions: finding positions, harness option TOML and project-tree paths.

use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use std::path::{Path, PathBuf};

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{
    PyBool, PyBoolMethods, PyDict, PyDictMethods, PyInt, PyList, PyString, PyStringMethods, PyTuple,
};
use pyo3::{IntoPyObject, pyfunction};
use sqlbuild_config::toml::main::toml_literal::toml_literal;
use sqlbuild_config::toml::models::TomlLiteral;
use sqlbuild_rules::project_tree::main::project_path_escapes::project_path_escapes;
use sqlbuild_rules::project_tree::main::project_tree_entries::project_tree_entries;
use sqlbuild_rules::project_tree::models::ProjectTreeEntry;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const BUILTINS_MODULE: &str = "builtins";
const DEFAULT_POSITION: i64 = 1;

/// `RuleUsageError(message)`.
fn usage_error(py: Python<'_>, message: String) -> PyErr {
    new_plain_error(py, ErrorId::RuleUsage, &message)
}

/// `RuleUsageError(prefix + str(value))`, keeping the value's text exactly as Python spells it.
pub(crate) fn usage_error_for(prefix: &str, value: &Bound<'_, PyAny>) -> PyErr {
    let py: Python<'_> = value.py();
    let message = value
        .str()
        .and_then(|text| PyString::new(py, prefix).as_any().add(text));
    match message.and_then(|message| {
        error_class(py, ErrorId::RuleUsage).and_then(|class| class.call1((message,)))
    }) {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// An explicit position, or `int(getattr(node, attribute, 1) or 1)`.
fn position<'py>(
    explicit: &Bound<'py, PyAny>,
    node: &Bound<'py, PyAny>,
    attribute: &str,
) -> PyResult<Bound<'py, PyAny>> {
    if !explicit.is_none() {
        return Ok(explicit.clone());
    }
    let py: Python<'py> = node.py();
    let value: Bound<'py, PyAny> = match native_field(node, attribute) {
        Ok(value) => value,
        Err(error) if error.is_instance_of::<PyAttributeError>(py) => {
            DEFAULT_POSITION.into_pyobject(py)?.into_any()
        }
        Err(error) => return Err(error),
    };
    let value: Bound<'py, PyAny> = if value.is_truthy()? {
        value
    } else {
        DEFAULT_POSITION.into_pyobject(py)?.into_any()
    };
    imported(py, BUILTINS_MODULE, "int")?.call1((value,))
}

/// The `(line, column)` of a finding at the project-relative `path`, from `node` when unset.
#[pyfunction]
#[pyo3(signature = (*, path, line, column, node))]
pub(crate) fn finding_position<'py>(
    path: &Bound<'py, PyAny>,
    line: &Bound<'py, PyAny>,
    column: &Bound<'py, PyAny>,
    node: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = path.py();
    if escapes(path)? {
        return Err(usage_error_for(
            "finding path must be project-relative: ",
            path,
        ));
    }
    let line: Bound<'py, PyAny> = position(line, node, "line")?;
    let column: Bound<'py, PyAny> = position(column, node, "column")?;
    if line.lt(DEFAULT_POSITION)? || column.lt(DEFAULT_POSITION)? {
        return Err(usage_error(
            py,
            "finding line and column must be positive".to_owned(),
        ));
    }
    Ok((line, column))
}

/// One rule-option value as a TOML literal, or the harness's usage error.
fn option_literal(value: &Bound<'_, PyAny>) -> PyResult<TomlLiteral> {
    if let Ok(flag) = value.cast::<PyBool>() {
        return Ok(TomlLiteral::Bool(flag.is_true()));
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(TomlLiteral::Integer(value.str()?.to_string()));
    }
    if let Ok(text) = value.cast::<PyString>() {
        return Ok(TomlLiteral::Text(text.to_str()?.to_owned()));
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        return Ok(TomlLiteral::Array(
            value
                .try_iter()?
                .map(|item| option_literal(&item?))
                .collect::<PyResult<_>>()?,
        ));
    }
    Err(usage_error(
        value.py(),
        format!(
            "RuleCase config value {} cannot be represented as a rule option",
            value.repr()?
        ),
    ))
}

/// The TOML text the rule harness writes for one `RuleCase` option value.
#[pyfunction]
pub(crate) fn rule_option_toml_value(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(toml_literal(&option_literal(value)?))
}

/// Reject a project path that is absolute or climbs out of the project.
#[pyfunction]
pub(crate) fn validate_project_relative(path: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    if escapes(path)? {
        return Err(usage_error_for("project path escapes the project: ", path));
    }
    Ok(())
}

/// Whether `str(path)` is absolute or climbs out of the project with `..`.
fn escapes(path: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(project_path_escapes(Path::new(
        &*path.str()?.to_string_lossy(),
    )))
}

/// Every supported path under the resolved project `root`, as sorted `ProjectPath`s.
#[pyfunction]
pub(crate) fn project_tree_paths<'py>(root: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = root.py();
    let root: PathBuf = root.extract()?;
    let mut paths: Vec<(Bound<'py, PyAny>, bool)> = project_tree_entries(&root)
        .into_iter()
        .map(|ProjectTreeEntry { relative, is_file }| {
            Ok((
                relative.into_os_string().into_pyobject(py)?.into_any(),
                is_file,
            ))
        })
        .collect::<PyResult<_>>()?;
    let mut failure: Option<PyErr> = None;
    paths.sort_by(|left, right| {
        left.0.compare(&right.0).unwrap_or_else(|error| {
            failure.get_or_insert(error);
            std::cmp::Ordering::Equal
        })
    });
    if let Some(error) = failure {
        return Err(error);
    }
    let nodes: Vec<Bound<'py, PyAny>> = paths
        .into_iter()
        .map(|(value, is_file)| {
            let keywords: Bound<'py, PyDict> = PyDict::new(py);
            keywords.set_item("value", value)?;
            keywords.set_item("is_file", is_file)?;
            keywords.set_item("is_dir", !is_file)?;
            new_record(py, RecordId::ProjectPath, (), Some(&keywords))
        })
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, nodes)
}
