use pyo3::prelude::{IntoPyObject, Py, PyAny, PyResult, Python};

/// Return normalized SQL in order, carrying each failure as the exception a single call raises.
pub(crate) fn normalization_results(
    py: Python<'_>,
    results: Vec<Result<String, String>>,
) -> PyResult<Vec<Py<PyAny>>> {
    results
        .into_iter()
        .map(|result| match result {
            Ok(sql) => Ok(sql.into_pyobject(py)?.into_any().unbind()),
            Err(error) => Ok(
                sqlbuild_python_base::boundary::main::compiler_error::compiler_error(error)
                    .into_value(py)
                    .into_any(),
            ),
        })
        .collect()
}
