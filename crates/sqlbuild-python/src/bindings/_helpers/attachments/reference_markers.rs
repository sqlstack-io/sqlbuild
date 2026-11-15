//! Reject executable SQL that still contains an unresolved reference call.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyString, PyStringMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_attachments::reference_markers::main::unresolved_reference_marker::unresolved_reference_marker;
use sqlbuild_attachments::sql_lexing::main::unclosed_message::unclosed_message;

use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// The reference function and diagnostic code of each marker kind, in the scanner's order.
const MARKER_KINDS: [(&str, &str); 5] = [
    ("__ref", "R001"),
    ("__seed", "R002"),
    ("__source", "R003"),
    ("__udf", "R004"),
    ("__table_fn", "R005"),
];

/// A `ValueError` carrying `message` and `code` attributes.
fn coded_value_error(py: Python<'_>, message: String, code: &str) -> PyResult<PyErr> {
    let error: PyErr = PyValueError::new_err(message.clone());
    let value = error.value(py);
    value.setattr("message", message)?;
    value.setattr("code", code)?;
    Ok(error)
}

/// Fail if executable SQL still contains unresolved reference markers or an unclosed construct.
#[pyfunction]
#[pyo3(signature = (*, sql, context))]
pub(crate) fn assert_no_unresolved_sql_markers(
    py: Python<'_>,
    sql: &Bound<'_, PyString>,
    context: &str,
) -> PyResult<()> {
    let text = sql.to_string_lossy();
    let outcome: (Option<usize>, Option<String>) = compiler_guard(|| {
        Ok(match unresolved_reference_marker(&text) {
            Ok(marker) => (marker, None),
            Err(construct) => (None, Some(unclosed_message(context, construct))),
        })
    })?;
    match outcome {
        (_, Some(unclosed)) => Err(PyErr::from_value(
            error_class(py, ErrorId::CompileInput)?.call1((unclosed,))?,
        )),
        (Some(marker), None) => {
            let (function, code) = MARKER_KINDS.get(marker).copied().ok_or_else(|| {
                PyValueError::new_err(format!("unknown reference marker kind {marker}"))
            })?;
            Err(coded_value_error(
                py,
                format!("{context} still contains unresolved {function}() markers"),
                code,
            )?)
        }
        (None, None) => Ok(()),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(assert_no_unresolved_sql_markers, module)?)?;
    Ok(())
}
