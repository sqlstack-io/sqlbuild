//! Simple top-level SELECT output names and their authored locations.

use crate::sqltext::main::location_rows::location_rows;
use crate::sqltext::models::ColumnLocation;
use pyo3::prelude::PyResult;
use pyo3::pyfunction;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;

/// Simple top-level SELECT output names and their authored locations.
#[pyfunction]
pub fn model_output_columns(
    contents: &str,
    implicit_aliases: bool,
) -> PyResult<Vec<ColumnLocation>> {
    compiler_guard(|| {
        Ok(location_rows(
            sqlbuild_sqltext::compiler::main::model_output_columns::model_output_columns(
                contents,
                implicit_aliases,
            ),
        ))
    })
}
