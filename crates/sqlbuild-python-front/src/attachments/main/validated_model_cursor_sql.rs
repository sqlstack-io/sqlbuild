//! One model query with canonical intrinsic calls, raising when the model may not use them.

use crate::attachments::_helpers::cursor_intrinsics;
use pyo3::Python;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// One model query with canonical intrinsic calls, raising when the model may not use them.
#[inline]
pub fn validated_model_cursor_sql(
    py: Python<'_>,
    sql: &str,
    (model_name, config_values): (&str, &Bound<'_, PyAny>),
) -> PyResult<String> {
    cursor_intrinsics::validated_model_cursor_sql(py, sql, (model_name, config_values))
}
