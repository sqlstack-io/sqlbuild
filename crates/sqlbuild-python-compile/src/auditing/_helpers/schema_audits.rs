//! Authored schema audit lists parsed natively into `SchemaAuditInstance` objects.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_model_config::errors::{ConfigError, ErrorClass};
use sqlbuild_model_config::header_metadata::main::parse_audit_list::parse_audit_list;
use sqlbuild_model_config::header_metadata::models::ParsedAudit;

use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_front::model_config::main::headers::audit_tuple::audit_tuple;
use sqlbuild_python_front::model_config::main::model_nodes::contract_classes::contract_classes;
use sqlbuild_python_front::model_config::models::ContractClasses;
use sqlbuild_python_front::model_config::models::PyNode;

/// A resource identity error as `ResourceIdentityError`, anything else as `fallback_class(message)`.
fn raised(
    py: Python<'_>,
    error: ConfigError,
    fallback_class: &Bound<'_, PyAny>,
) -> PyResult<PyErr> {
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    let class: Bound<'_, PyAny> = match error.class {
        ErrorClass::ResourceIdentity => {
            if let Some(help) = error.help {
                keywords.set_item("help", help)?;
            }
            error_class(py, ErrorId::ResourceIdentity)?
        }
        _ => fallback_class.clone(),
    };
    Ok(PyErr::from_value(
        class.call((error.message,), Some(&keywords))?,
    ))
}

/// Parse an authored audit list, treating null as empty only when requested.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn parse_audit_instances<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let request = Request::new("parse_audit_instances", request)?;
    let py: Python<'py> = request.py();
    let raw_audits: Bound<'py, PyAny> = request.get("raw_audits")?;
    let path: String = request.get("file_path")?.str()?.extract()?;
    let label: String = request.get("label")?.extract()?;
    let error_class: Bound<'py, PyAny> = request.get("error_class")?;
    let null_as_empty: bool = request.get("null_as_empty")?.is_truthy()?;
    let node: PyNode<'py> = PyNode(raw_audits);
    let parsed: Vec<ParsedAudit<PyNode<'py>>> =
        match parse_audit_list(Some(&node), &path, &label, null_as_empty) {
            Ok(parsed) => parsed,
            Err(error) => return Err(raised(py, error, &error_class)?),
        };
    let classes: ContractClasses<'py> = contract_classes(py)?.extract()?;
    audit_tuple(py, &classes, &parsed)
}
