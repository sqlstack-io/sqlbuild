//! Logical SQL refs outside comments and quotes, reporting rejected calls as P012.

use crate::sqltext::_helpers::sql_references::REFERENCE_CALL_SYNTAX_CODE;
use crate::sqltext::_helpers::sql_references::scanned;
use crate::sqltext::_helpers::sql_references::text;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;
use pyo3::{IntoPyObject, pyfunction};
use sqlbuild_python_base::compile_context::main::diagnostic_collector::report_compile_diagnostic::report_compile_diagnostic;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Logical SQL refs outside comments and quotes, reporting rejected calls as P012.
#[pyfunction(name = "extract_sql_references")]
#[pyo3(signature = (*, sql, syntax))]
pub fn extract_references<'py>(
    sql: &str,
    syntax: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = syntax.py();
    let (references, invalid_calls) = scanned(py, sql, syntax)?;
    for call in invalid_calls {
        let diagnostic: Bound<'py, PyAny> = new_record_with(
            py,
            RecordId::CompilerDiagnostic,
            &[
                (
                    "phase",
                    enum_member(py, RecordId::DiagnosticPhase, "COMPILE")?,
                ),
                (
                    "severity",
                    enum_member(py, RecordId::DiagnosticSeverity, "ERROR")?,
                ),
                ("code", text(py, REFERENCE_CALL_SYNTAX_CODE)?),
                ("message", native_field(&call, "message")?),
                ("help", native_field(&call, "help")?),
            ],
        )?;
        report_compile_diagnostic(
            (REFERENCE_CALL_SYNTAX_CODE, "", native_field(&call, "call")?)
                .into_pyobject(py)?
                .as_any(),
            &diagnostic,
        )?;
    }
    PyTuple::new(py, references)
}
