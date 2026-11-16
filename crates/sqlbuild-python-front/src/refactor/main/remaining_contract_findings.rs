//! The findings an edit leaves, plus an ownership-conflict finding when the edit was refused.

use crate::refactor::_helpers::contract_edits::RESOLVED_KINDS;
use crate::refactor::_helpers::contract_edits::adoption_error;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// The findings an edit leaves, plus an ownership-conflict finding when the edit was refused.
#[pyfunction]
#[pyo3(signature = (*, findings, overwrite, conflict))]
pub fn remaining_contract_findings<'py>(
    py: Python<'py>,
    findings: &Bound<'py, PyAny>,
    overwrite: bool,
    conflict: Option<String>,
) -> PyResult<Bound<'py, PyTuple>> {
    let findings: Vec<Bound<'py, PyAny>> = findings.try_iter()?.collect::<PyResult<_>>()?;
    if let Some(conflict) = conflict {
        let Some(first) = findings.first() else {
            return Err(adoption_error(
                py,
                "contract ownership conflict requires resource evidence".to_owned(),
            ));
        };
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        keywords.set_item("resource_type", native_field(first, "resource_type")?)?;
        keywords.set_item("resource_name", native_field(first, "resource_name")?)?;
        keywords.set_item(
            "kind",
            enum_member(py, RecordId::ContractFindingKind, "OWNERSHIP_CONFLICT")?,
        )?;
        keywords.set_item("message", conflict)?;
        let ownership: Bound<'py, PyAny> =
            new_record(py, RecordId::ContractFinding, (), Some(&keywords))?;
        let mut remaining: Vec<Bound<'py, PyAny>> = findings.clone();
        remaining.push(ownership);
        return PyTuple::new(py, remaining);
    }
    let mut remaining: Vec<Bound<'py, PyAny>> = Vec::new();
    for finding in findings {
        let kind: String = native_field(&finding, "kind")?.str()?.extract()?;
        let resolved: bool = RESOLVED_KINDS.0.contains(&kind.as_str())
            || (overwrite && RESOLVED_KINDS.1.contains(&kind.as_str()));
        if !resolved {
            remaining.push(finding);
        }
    }
    PyTuple::new(py, remaining)
}
