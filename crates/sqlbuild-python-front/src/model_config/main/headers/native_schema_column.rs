//! Native schema column.

use crate::model_config::_helpers::headers::header_metadata;
use crate::model_config::models::ContractClasses;
use crate::model_config::models::HeaderColumn;
use crate::model_config::types::HeaderValue;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use sqlbuild_model_config::header_metadata::models::ParsedColumn;

/// Native schema column.
#[inline]
pub fn native_schema_column<'py, N: HeaderValue>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    column: &ParsedColumn<N>,
    locations: &Bound<'py, PyDict>,
) -> PyResult<HeaderColumn> {
    header_metadata::native_schema_column(py, classes, column, locations)
}
