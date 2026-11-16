//! Match, tokenize and parse model headers natively.

use crate::sqltext::main::location_rows::location_rows;
use crate::sqltext::models::ColumnLocation;
use pyo3::prelude::PyResult;
use pyo3::pyfunction;

use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;

/// The authored locations of a header's column names, or the header's syntax error.
#[pyfunction]
pub(crate) fn model_header_column_locations(
    contents: &str,
    header: &str,
    header_start: usize,
) -> PyResult<(Option<Vec<ColumnLocation>>, Option<String>)> {
    compiler_guard(|| {
        Ok(
            match sqlbuild_sqltext::compiler::main::model_header_column_locations::model_header_column_locations(
                contents,
                header,
                header_start,
            ) {
                Ok(locations) => (Some(location_rows(locations)), None),
                Err(error) => (None, Some(error)),
            },
        )
    })
}
