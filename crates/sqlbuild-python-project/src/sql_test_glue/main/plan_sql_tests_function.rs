//! `plan_sql_tests` as a function of `module`.

use crate::sql_test_glue::_helpers::plan_assembly;

/// `plan_sql_tests` as a function of `module`.
#[inline]
pub fn plan_sql_tests_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    plan_assembly::plan_sql_tests_function(module)
}
