//! Contract adoption's MODEL header and YAML source edits, computed natively.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use sqlbuild_refactor::refactoring::errors::{RefactorError, RefactorErrorKind};
use sqlbuild_refactor::refactoring::models::ContractEdit;
use sqlbuild_refactor::refactoring::types::PhysicalColumn;

use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

/// Finding kinds an edit resolves without overwrite, then the ones overwrite also resolves.
pub(crate) const RESOLVED_KINDS: ([&str; 1], [&str; 2]) = (
    ["missing_declaration"],
    ["type_mismatch", "missing_physical_column"],
);

/// `(edited contents, ownership conflict, ContractAdoptionError message)`.
pub(crate) type EditRow = (Option<String>, Option<String>, Option<String>);
pub(crate) fn edit_row(result: Result<ContractEdit, RefactorError>) -> PyResult<EditRow> {
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
pub(crate) fn adoption_error(py: Python<'_>, message: String) -> PyErr {
    new_plain_error(py, ErrorId::ContractAdoption, &message)
}

/// `(edited contents, ownership conflict)`, raising the edit's adoption failure.
pub(crate) fn edit_outcome(
    py: Python<'_>,
    row: EditRow,
) -> PyResult<(Option<String>, Option<String>)> {
    let (contents, conflict, failure) = row;
    match failure {
        Some(failure) => Err(adoption_error(py, failure)),
        None => Ok((contents, conflict)),
    }
}

pub(crate) fn physical_rows(columns: &Bound<'_, PyAny>) -> PyResult<Vec<PhysicalColumn>> {
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
