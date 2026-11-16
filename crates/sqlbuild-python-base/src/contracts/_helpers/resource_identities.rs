//! Canonical snake_case resource identity checks and SQL function node types.

use crate::records::main::construct::error_class::error_class;
use crate::records::types::ErrorId;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyString, PyStringMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_model_config::resource_identities::main::resource_identity_failure::resource_identity_failure;

const TABLE_FUNCTION_NODE_TYPE: &str = "table_fn";
const UDF_NODE_TYPE: &str = "udf";

/// Reject one identity unless it uses canonical lowercase snake_case.
#[pyfunction]
#[pyo3(signature = (*, name, kind, path, private_identity = false))]
pub(crate) fn validate_resource_identity(
    name: &Bound<'_, PyString>,
    kind: &str,
    path: &Bound<'_, PyAny>,
    private_identity: bool,
) -> Result<(), PyErr> {
    let py: Python<'_> = name.py();
    let text = name.to_string_lossy();
    let Some((corrected_name, help_text)) = resource_identity_failure(&text, private_identity)
    else {
        return Ok(());
    };
    let message: String = format!(
        "Invalid {kind} identity '{text}' in {}; use snake_case '{corrected_name}'",
        path.str()?
    );
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item("help", help_text)?;
    let error: Bound<'_, PyAny> =
        error_class(py, ErrorId::ResourceIdentity)?.call((message,), Some(&keywords))?;
    Err(PyErr::from_value(error))
}

/// The public node type of a SQL function: a table function when it returns columns.
#[pyfunction]
#[pyo3(signature = (*, return_columns))]
fn function_node_type(return_columns: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    Ok(if return_columns.is_truthy()? {
        TABLE_FUNCTION_NODE_TYPE
    } else {
        UDF_NODE_TYPE
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(validate_resource_identity, module)?)?;
    module.add_function(wrap_pyfunction!(function_node_type, module)?)
}
