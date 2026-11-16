//! `plan_compiled_sql_test_artifacts` as a function of `module`.

use crate::sql_test_glue::_helpers::plan::plan_driver;

/// `plan_compiled_sql_test_artifacts` as a function of `module`.
#[inline]
pub fn plan_compiled_sql_test_artifacts_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    plan_driver::plan_compiled_sql_test_artifacts_function(module)
}
