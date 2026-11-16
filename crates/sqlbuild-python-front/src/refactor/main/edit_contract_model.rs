//! The model's MODEL header with physical columns adopted, or the ownership conflict.

use crate::refactor::_helpers::contract_edits::EditRow;
use crate::refactor::_helpers::contract_edits::edit_outcome;
use crate::refactor::_helpers::contract_edits::edit_row;
use crate::refactor::_helpers::contract_edits::physical_rows;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_refactor::refactoring::main::edit_model_contract::edit_model_contract;
use sqlbuild_refactor::refactoring::models::{ContractModelColumn, ContractModelRequest};

/// The model's MODEL header with physical columns adopted, or the ownership conflict.
#[pyfunction]
#[pyo3(signature = (*, model, physical_columns, overwrite))]
pub fn edit_contract_model(
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
