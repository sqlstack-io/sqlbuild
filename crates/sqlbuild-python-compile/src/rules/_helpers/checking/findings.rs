//! Rule findings converted between the Python `Finding` model and native rows and payloads.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use serde_json::Value;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_rules::models::Fault;

const PATHLIB_MODULE: &str = "pathlib";
const PATH_CLASS: &str = "Path";
const INVALID_FINDING: &str = "native rules engine returned an invalid finding";

/// The Python `Finding` and `Path` classes, imported once per conversion batch.
pub(crate) struct FindingClasses<'py> {
    finding: Bound<'py, PyAny>,
    path: Bound<'py, PyAny>,
}

impl<'py> FindingClasses<'py> {
    pub(crate) fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            finding: record_class(py, RecordId::Finding)?,
            path: py.import(PATHLIB_MODULE)?.getattr(PATH_CLASS)?,
        })
    }

    /// One finding from its native fault row, optionally marked fixable.
    pub(crate) fn finding_of_fault(
        &self,
        fault: Fault,
        fixable: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = self.finding.py();
        let arguments: Bound<'py, PyDict> = PyDict::new(py);
        arguments.set_item("code", fault.code)?;
        arguments.set_item("path", self.path.call1((fault.path,))?)?;
        arguments.set_item("line", fault.line)?;
        arguments.set_item("column", fault.column)?;
        arguments.set_item("message", fault.message)?;
        arguments.set_item("remediation", fault.remediation)?;
        arguments.set_item("unevaluated", fault.unevaluated)?;
        if fixable {
            arguments.set_item("fixable", true)?;
        }
        self.finding.call((), Some(&arguments))
    }

    /// One finding from a stored or custom-rule payload, or `None` when the payload is malformed.
    pub(crate) fn finding_of_payload(&self, value: &Value) -> PyResult<Option<Bound<'py, PyAny>>> {
        let Some(payload) = value.as_object() else {
            return Ok(None);
        };
        let text = |key: &str| payload.get(key).and_then(Value::as_str).map(str::to_owned);
        let number = |key: &str| payload.get(key).and_then(Value::as_u64);
        let flag = |key: &str| payload.get(key).and_then(Value::as_bool) == Some(true);
        let (Some(code), Some(path), Some(line), Some(column), Some(message), Some(remediation)) = (
            text("code"),
            text("path"),
            number("line"),
            number("column"),
            text("message"),
            text("remediation"),
        ) else {
            return Ok(None);
        };
        let fault = Fault {
            unevaluated: flag("unevaluated"),
            code,
            path,
            line,
            column,
            message,
            remediation,
        };
        Ok(Some(self.finding_of_fault(fault, flag("fixable"))?))
    }
}

/// The native fault row of one Python `Finding`.
pub(crate) fn finding_fault(finding: &Bound<'_, PyAny>) -> PyResult<Fault> {
    Ok(Fault {
        unevaluated: native_field(finding, "unevaluated")?.extract()?,
        code: native_field(finding, "code")?.extract()?,
        path: native_field(finding, "path")?
            .call_method0("as_posix")?
            .extract()?,
        line: native_field(finding, "line")?.extract()?,
        column: native_field(finding, "column")?.extract()?,
        message: native_field(finding, "message")?.extract()?,
        remediation: native_field(finding, "remediation")?.extract()?,
    })
}

/// The `RulesError` a malformed native or custom-rule finding raises.
pub(crate) fn invalid_finding(py: Python<'_>) -> PyErr {
    let raised = || -> PyResult<PyErr> {
        let class: Bound<'_, PyAny> = error_class(py, ErrorId::Rules)?;
        Ok(PyErr::from_value(class.call1((INVALID_FINDING,))?))
    };
    raised().unwrap_or_else(|error| error)
}
