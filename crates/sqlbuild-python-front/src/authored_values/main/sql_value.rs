//! A Python `SqlValue` for a native one.

use crate::authored_values::_helpers::sql_values;
use crate::authored_values::models::SqlValueClasses;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_core::models::SqlValue;

/// A Python `SqlValue` for a native one.
#[inline]
pub fn sql_value<'py>(
    value: &SqlValue,
    classes: &SqlValueClasses<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    sql_values::sql_value(value, classes)
}
