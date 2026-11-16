//! A declined body's `format-unsafe` fault, or a `format-unavailable` warning for an unformatted dialect.

use crate::rules::_helpers::lint::lint_pipeline::NATIVE_ENGINE;
use crate::rules::_helpers::lint::lint_text::FORMAT_UNAVAILABLE_CODE;
use crate::rules::_helpers::lint::lint_text::FORMAT_UNAVAILABLE_PREFIX;
use crate::rules::_helpers::lint::lint_text::FORMAT_UNAVAILABLE_REMEDIATION;
use crate::rules::_helpers::lint::lint_text::FORMAT_UNSAFE_CODE;
use crate::rules::_helpers::lint::lint_text::FORMAT_UNSAFE_PREFIX;
use crate::rules::_helpers::lint::lint_text::FORMAT_UNSAFE_REMEDIATION;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_rules::lint_text::models::FormatFault;

/// A declined body's `format-unsafe` fault, or a `format-unavailable` warning for an unformatted dialect.
pub fn unsafe_fault<'py>(
    file_path: &Bound<'py, PyAny>,
    fault: &FormatFault,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = file_path.py();
    let (severity, code, prefix): (&str, &str, &str) = if fault.unavailable {
        (
            "WARNING",
            FORMAT_UNAVAILABLE_CODE,
            FORMAT_UNAVAILABLE_PREFIX,
        )
    } else {
        ("FAULT", FORMAT_UNSAFE_CODE, FORMAT_UNSAFE_PREFIX)
    };
    let remediation: &str = if fault.unavailable {
        FORMAT_UNAVAILABLE_REMEDIATION
    } else {
        FORMAT_UNSAFE_REMEDIATION
    };
    let message: String = format!("{prefix}: {}", fault.reason);
    let severity: Bound<'py, PyAny> = enum_member(py, RecordId::LintSeverity, severity)?;
    new_record_with(
        py,
        RecordId::LintViolation,
        &[
            ("file_path", file_path.clone()),
            ("line", fault.line.into_pyobject(py)?.into_any()),
            ("column", 1_usize.into_pyobject(py)?.into_any()),
            ("code", PyString::new(py, code).into_any()),
            ("message", PyString::new(py, &message).into_any()),
            ("severity", severity),
            ("engine", PyString::new(py, NATIVE_ENGINE).into_any()),
            ("remediation", PyString::new(py, remediation).into_any()),
        ],
    )
}
