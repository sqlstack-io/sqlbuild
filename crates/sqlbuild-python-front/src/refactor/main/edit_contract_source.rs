//! The source's YAML declaration with physical columns adopted, or the ownership conflict.

use crate::refactor::_helpers::contract_edits::EditRow;
use crate::refactor::_helpers::contract_edits::edit_outcome;
use crate::refactor::_helpers::contract_edits::edit_row;
use crate::refactor::_helpers::contract_edits::physical_rows;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_refactor::refactoring::main::edit_source_contract::edit_source_contract;
use sqlbuild_refactor::refactoring::models::ContractSourceRequest;

/// The source's YAML declaration with physical columns adopted, or the ownership conflict.
#[pyfunction]
#[pyo3(signature = (*, source, physical_columns, overwrite, contents = None))]
pub fn edit_contract_source(
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
