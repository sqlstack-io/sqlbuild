//! `plan_sql_tests` as a function of `module`.

use crate::sql_test_glue::_helpers::plan::plan_assembly::plan_sql_tests;

/// `plan_sql_tests` as a function of `module`.
pub fn plan_sql_tests_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(plan_sql_tests, module)
}
