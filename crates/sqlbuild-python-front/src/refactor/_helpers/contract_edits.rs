//! Contract adoption's MODEL header and YAML source edits, computed natively.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use sqlbuild_refactor::refactoring::errors::{RefactorError, RefactorErrorKind};
use sqlbuild_refactor::refactoring::main::edit_model_contract::edit_model_contract;
use sqlbuild_refactor::refactoring::main::edit_source_contract::edit_source_contract;
use sqlbuild_refactor::refactoring::models::{
    ContractEdit, ContractModelColumn, ContractModelRequest, ContractSourceRequest,
};
use sqlbuild_refactor::refactoring::types::PhysicalColumn;

use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

/// Finding kinds an edit resolves without overwrite, then the ones overwrite also resolves.
const RESOLVED_KINDS: ([&str; 1], [&str; 2]) = (
    ["missing_declaration"],
    ["type_mismatch", "missing_physical_column"],
);

/// `(edited contents, ownership conflict, ContractAdoptionError message)`.
type EditRow = (Option<String>, Option<String>, Option<String>);
fn edit_row(result: Result<ContractEdit, RefactorError>) -> PyResult<EditRow> {
    match result {
        Ok(ContractEdit::Edited(contents)) => Ok((Some(contents), None, None)),
        Ok(ContractEdit::Conflict(conflict)) => Ok((None, Some(conflict), None)),
        Err(error) if error.kind == RefactorErrorKind::Edit => {
            Ok((None, None, Some(error.message)))
        }
        Err(error) => Err(PyValueError::new_err(error.message)),
    }
}

/// `ContractAdoptionError(message)`.
fn adoption_error(py: Python<'_>, message: String) -> PyErr {
    new_plain_error(py, ErrorId::ContractAdoption, &message)
}

/// `(edited contents, ownership conflict)`, raising the edit's adoption failure.
fn edit_outcome(py: Python<'_>, row: EditRow) -> PyResult<(Option<String>, Option<String>)> {
    let (contents, conflict, failure) = row;
    match failure {
        Some(failure) => Err(adoption_error(py, failure)),
        None => Ok((contents, conflict)),
    }
}

fn physical_rows(columns: &Bound<'_, PyAny>) -> PyResult<Vec<PhysicalColumn>> {
    let mut rows: Vec<PhysicalColumn> = Vec::new();
    for column in columns.try_iter()? {
        let column: Bound<'_, PyAny> = column?;
        rows.push((
            native_field(&column, "name")?.extract()?,
            native_field(&column, "type")?.extract()?,
        ));
    }
    Ok(rows)
}

/// The model's MODEL header with physical columns adopted, or the ownership conflict.
#[pyfunction]
#[pyo3(signature = (*, model, physical_columns, overwrite))]
pub(crate) fn edit_contract_model(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    physical_columns: &Bound<'_, PyAny>,
    overwrite: bool,
) -> PyResult<(Option<String>, Option<String>)> {
    let schema: Bound<'_, PyAny> = native_field(model, "schema_entry")?;
    let mut columns: Vec<ContractModelColumn> = Vec::new();
    let mut dynamic_patterns: Vec<Option<String>> = Vec::new();
    let shared_schema: bool =
        !schema.is_none() && !native_field(&schema, "model_schema")?.is_none();
    if !schema.is_none() {
        for column in native_field(&schema, "columns")?.try_iter()? {
            let column: Bound<'_, PyAny> = column?;
            let location: Bound<'_, PyAny> = native_field(&column, "location")?;
            let location_path: Option<String> = if location.is_none() {
                None
            } else {
                Some(native_field(&location, "path")?.str()?.extract()?)
            };
            columns.push(ContractModelColumn {
                name: native_field(&column, "name")?.extract()?,
                declared_type: native_field(&column, "type")?.extract()?,
                location_path,
            });
        }
        for family in native_field(&schema, "dynamic_columns")?.try_iter()? {
            dynamic_patterns.push(native_field(&family?, "name_pattern")?.extract()?);
        }
    }
    let request = ContractModelRequest {
        contents: native_field(model, "authored_sql")?.extract()?,
        relative_path: native_field(model, "relative_path")?.str()?.extract()?,
        shared_schema,
        columns,
        dynamic_patterns,
        physical_columns: physical_rows(physical_columns)?,
        overwrite,
    };
    let row: EditRow = compiler_guard(|| edit_row(edit_model_contract(&request)))?;
    edit_outcome(py, row)
}

/// The source's YAML declaration with physical columns adopted, or the ownership conflict.
#[pyfunction]
#[pyo3(signature = (*, source, physical_columns, overwrite, contents = None))]
pub(crate) fn edit_contract_source(
    source: &Bound<'_, PyAny>,
    physical_columns: &Bound<'_, PyAny>,
    overwrite: bool,
    contents: Option<String>,
) -> PyResult<(Option<String>, Option<String>)> {
    let discovered_contents: String =
        native_field(&native_field(source, "source_file")?, "contents")?.extract()?;
    let mut columns: Vec<(String, Option<String>)> = Vec::new();
    for column in native_field(&native_field(source, "source_entry")?, "columns")?.try_iter()? {
        let column: Bound<'_, PyAny> = column?;
        columns.push((
            native_field(&column, "name")?.extract()?,
            native_field(&column, "type")?.extract()?,
        ));
    }
    let request = ContractSourceRequest {
        contents: contents.unwrap_or_else(|| discovered_contents.clone()),
        discovered_contents,
        name: native_field(source, "name")?.extract()?,
        columns,
        physical_columns: physical_rows(physical_columns)?,
        overwrite,
    };
    let row: EditRow = compiler_guard(|| edit_row(Ok(edit_source_contract(&request))))?;
    edit_outcome(source.py(), row)
}

/// The findings an edit leaves, plus an ownership-conflict finding when the edit was refused.
#[pyfunction]
#[pyo3(signature = (*, findings, overwrite, conflict))]
pub(crate) fn remaining_contract_findings<'py>(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(edit_contract_model, module)?)?;
    module.add_function(wrap_pyfunction!(edit_contract_source, module)?)?;
    module.add_function(wrap_pyfunction!(remaining_contract_findings, module)?)?;
    Ok(())
}
