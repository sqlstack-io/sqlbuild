//! An authored Python value read by exact type, as `normalize_sql_value` dispatches.

use crate::authored_values::_helpers::sql_values;
use crate::authored_values::models::SqlValueClasses;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_model_config::sql_values::models::RawSqlValue;

/// An authored Python value read by exact type, as `normalize_sql_value` dispatches.
#[inline]
pub fn raw_value(value: &Bound<'_, PyAny>, classes: &SqlValueClasses<'_>) -> PyResult<RawSqlValue> {
    sql_values::raw_value(value, classes)
}
