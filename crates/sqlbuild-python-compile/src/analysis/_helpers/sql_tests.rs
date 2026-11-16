//! Extract, plan and render SQL tests natively.

use pyo3::prelude::{PyResult, Python};
use pyo3::pyfunction;

use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::types::CompilerDetach;

#[pyfunction]
pub(crate) fn extract_sql_tests_json(py: Python<'_>, request_json: &str) -> PyResult<String> {
    py.compiler_detach(|| {
        sqlbuild_analysis::compiler::main::sql_test_extraction::extract_batch_json(request_json)
    })
    .map_err(value_error)
}
