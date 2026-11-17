//! A compile that stopped on a raised error, as one JSON diagnostic report.

use crate::compile_command::_helpers::invocation::report::ERROR_SEVERITY;
use crate::compile_command::_helpers::invocation::report::any;
use crate::compile_command::_helpers::invocation::report::diagnostic_json;
use crate::compile_command::_helpers::invocation::report::dict;
use crate::compile_command::_helpers::invocation::report::sqlbuild_version;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyList};
use sqlbuild_python_base::compile_outputs::main::emit_orjson_report::emit_orjson_report;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// A compile that stopped on a raised error, as one JSON diagnostic report.
#[pyfunction]
#[pyo3(signature = (*, code, message, help_text, location=None))]
pub fn format_compile_error_json<'py>(
    code: Bound<'py, PyAny>,
    message: Bound<'py, PyAny>,
    help_text: Bound<'py, PyAny>,
    location: Option<Bound<'py, PyAny>>,
) -> PyResult<String> {
    let py: Python<'py> = code.py();
    let location: Bound<'py, PyAny> = location.unwrap_or_else(|| py.None().into_bound(py));
    let (path, line, column) = if location.is_none() {
        (location.clone(), location.clone(), location.clone())
    } else {
        (
            native_field(&location, "path")?,
            native_field(&location, "line")?,
            native_field(&location, "column")?,
        )
    };
    let diagnostic: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::CompilerDiagnostic,
        &[
            ("phase", any(py, "compile")?),
            ("severity", any(py, ERROR_SEVERITY)?),
            ("code", code),
            ("message", message),
            ("path", path),
            ("line", line),
            ("column", column),
            ("location", location),
            ("help", help_text),
        ],
    )?;
    let report: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("version", sqlbuild_version(py)?),
            ("command", any(py, "compile")?),
            ("offline", any(py, true)?),
            ("has_errors", any(py, true)?),
            ("stopped", any(py, true)?),
            (
                "diagnostics",
                PyList::new(py, [diagnostic_json(&diagnostic)?])?.into_any(),
            ),
        ],
    )?;
    emit_orjson_report(report.as_any())
}
