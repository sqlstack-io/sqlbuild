//! Parse model headers.

use crate::sqltext::main::optional_authored_value_to_python::optional_authored_value_to_python;
use crate::sqltext::types::ParsedModelHeader;
use pyo3::prelude::{PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::types::CompilerDetach;

#[pyfunction]
pub fn parse_model_headers(
    py: Python<'_>,
    headers: Vec<String>,
) -> PyResult<Vec<ParsedModelHeader>> {
    let parsed = py
        .compiler_detach(|| {
            sqlbuild_sqltext::compiler::main::model_header_parsing::parse_batch(&headers)
        })
        .map_err(value_error)?;
    parsed
        .into_iter()
        .map(|(value, offsets, error)| {
            Ok((
                optional_authored_value_to_python(py, value.as_ref())?,
                offsets,
                error,
            ))
        })
        .collect()
}
