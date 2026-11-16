//! `plan_sql_tests_natively` as a function of `module`.

use crate::sql_test_glue::_helpers::plan::plan_driver;

/// `plan_sql_tests_natively` as a function of `module`.
#[inline]
pub fn plan_sql_tests_natively_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    plan_driver::plan_sql_tests_natively_function(module)
}
