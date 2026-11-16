//! Extract, plan and render SQL tests natively.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::types::CompilerDetach;

#[pyfunction]
fn extract_sql_tests_json(py: Python<'_>, request_json: &str) -> PyResult<String> {
    py.compiler_detach(|| {
        sqlbuild_analysis::compiler::main::sql_test_extraction::extract_batch_json(request_json)
    })
    .map_err(value_error)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(extract_sql_tests_json, module)?)?;
    Ok(())
}
