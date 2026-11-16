//! The `SchemaColumn` tuple of one authored columns mapping, raising Python's error for it.

use crate::model_config::_helpers::headers::header_columns;
use crate::model_config::_helpers::headers::header_columns::ColumnOptions;
use crate::model_config::_helpers::headers::header_columns::ColumnSite;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// The `SchemaColumn` tuple of one authored columns mapping, raising Python's error for it.
#[inline]
pub fn schema_columns<'py>(
    raw_columns: &Bound<'py, PyAny>,
    site: ColumnSite<'_>,
    options: ColumnOptions,
    column_locations: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyTuple>> {
    header_columns::schema_columns(raw_columns, site, options, column_locations)
}
