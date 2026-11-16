//! A Python `SqlValue` for a native one.

use crate::authored_values::_helpers::sql_values::logical_type;
use crate::authored_values::_helpers::sql_values::payload;
use crate::authored_values::models::SqlValueClasses;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::models::SqlValue;

/// A Python `SqlValue` for a native one.
pub fn sql_value<'py>(
    value: &SqlValue,
    classes: &SqlValueClasses<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    classes.sql_value.call1((
        logical_type(&value.logical_type(), classes)?,
        payload(value, classes)?,
    ))
}
