//! Fail if executable SQL still contains unresolved reference markers or an unclosed construct.

use crate::attachments::_helpers::reference_markers::MARKER_KINDS;
use crate::attachments::_helpers::reference_markers::coded_value_error;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyString, PyStringMethods};
use sqlbuild_attachments::reference_markers::main::unresolved_reference_marker::unresolved_reference_marker;
use sqlbuild_attachments::sql_lexing::main::unclosed_message::unclosed_message;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// Fail if executable SQL still contains unresolved reference markers or an unclosed construct.
#[pyfunction]
#[pyo3(signature = (*, sql, context))]
pub fn assert_no_unresolved_sql_markers(
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
