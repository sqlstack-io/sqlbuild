//! `sql_test_model_chain_names_by_key` as a function of `module`.

use crate::sql_test_glue::_helpers::plan_driver;

/// `sql_test_model_chain_names_by_key` as a function of `module`.
#[inline]
pub fn sql_test_model_chain_names_by_key_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    plan_driver::sql_test_model_chain_names_by_key_function(module)
}
