//! The `SchemaColumn` a native column projects to when its location names `location`'s path.

use crate::model_config::_helpers::headers::header_metadata;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_discovery::model_files::models::NativeColumn;
use sqlbuild_discovery::models::LineColumnSpan;

/// The `SchemaColumn` a native column projects to when its location names `location`'s path.
#[inline]
pub fn native_column_record<'py>(
    py: Python<'py>,
    column: &NativeColumn,
    location: Option<(Bound<'py, PyAny>, LineColumnSpan)>,
) -> PyResult<Bound<'py, PyAny>> {
    header_metadata::native_column_record(py, column, location)
}
