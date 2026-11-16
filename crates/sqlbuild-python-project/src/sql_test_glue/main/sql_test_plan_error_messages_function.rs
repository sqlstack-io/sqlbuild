//! `sql_test_plan_error_messages` as a function of `module`.

use crate::sql_test_glue::_helpers::plan_driver;

/// `sql_test_plan_error_messages` as a function of `module`.
#[inline]
pub fn sql_test_plan_error_messages_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    plan_driver::sql_test_plan_error_messages_function(module)
}
