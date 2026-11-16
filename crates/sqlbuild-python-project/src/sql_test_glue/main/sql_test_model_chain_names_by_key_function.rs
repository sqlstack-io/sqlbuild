//! `sql_test_model_chain_names_by_key` as a function of `module`.

use crate::sql_test_glue::main::sql_test_model_chain_names_by_key::sql_test_model_chain_names_by_key;

/// `sql_test_model_chain_names_by_key` as a function of `module`.
pub fn sql_test_model_chain_names_by_key_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(sql_test_model_chain_names_by_key, module)
}
