//! Column name.

use crate::discovery::_helpers::project::project_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Column name.
#[inline]
pub fn column_name(column: &Bound<'_, PyAny>) -> PyResult<String> {
    project_facts::column_name(column)
}
