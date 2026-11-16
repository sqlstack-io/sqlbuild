//! `CliNamespace`: the mutable attribute namespace every `sqb` command reads its options from.

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyInt, PyList, PyString};
use pyo3::{IntoPyObjectExt, pyclass, pymethods};
use sqlbuild_cli::argv::main::cli_namespace_defaults::cli_namespace_defaults;
use sqlbuild_cli::argv::models::NamespaceDefault;

use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::types::RecordId;

/// Typed namespace for all CLI arguments across all commands.
#[pyclass(module = "sqlbuild._native", dict, frozen)]
pub(crate) struct CliNamespace;

fn default_object(py: Python<'_>, default: NamespaceDefault) -> PyResult<Bound<'_, PyAny>> {
    match default {
        NamespaceDefault::None => Ok(py.None().into_bound(py)),
        NamespaceDefault::Bool(value) => Ok(PyBool::new(py, value).to_owned().into_any()),
        NamespaceDefault::Int(value) => Ok(PyInt::new(py, value).into_any()),
        NamespaceDefault::Text(value) => Ok(PyString::new(py, value).into_any()),
        NamespaceDefault::EmptyList => Ok(PyList::empty(py).into_any()),
        NamespaceDefault::EmptyDict => Ok(PyDict::new(py).into_any()),
        NamespaceDefault::CompileLineageFast => {
            enum_member(py, RecordId::CompileLineageMode, "FAST")
        }
        NamespaceDefault::ColumnLineageRich => enum_member(py, RecordId::ColumnLineageMode, "RICH"),
    }
}

/// A namespace holding a fresh copy of every default as an instance attribute.
pub(crate) fn new_cli_namespace(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let namespace: Bound<'_, PyAny> = Bound::new(py, CliNamespace)?.into_any();
    for (name, default) in cli_namespace_defaults() {
        namespace.setattr(*name, default_object(py, *default)?)?;
    }
    Ok(namespace)
}

#[pymethods]
impl CliNamespace {
    #[new]
    fn new() -> Self {
        Self
    }

    /// A default not yet stored on this namespace, stored on first read.
    pub(crate) fn __getattr__<'py>(
        slf: &Bound<'py, Self>,
        name: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = slf.py();
        let Some((_, default)) = cli_namespace_defaults()
            .iter()
            .find(|(attribute, _)| *attribute == name)
        else {
            return Err(PyAttributeError::new_err(format!(
                "'CliNamespace' object has no attribute {}",
                name.into_bound_py_any(py)?.repr()?
            )));
        };
        let value: Bound<'py, PyAny> = default_object(py, *default)?;
        slf.as_any().setattr(name, &value)?;
        Ok(value)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CliNamespace>()
}
