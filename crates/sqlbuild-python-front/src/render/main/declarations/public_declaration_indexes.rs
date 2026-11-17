//! Every public enum and constant by name, collision-checked.

use crate::render::_helpers::declarations::declaration_indexes::index_declarations;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Every public enum and constant by name, collision-checked.
#[pyfunction]
pub fn public_declaration_indexes<'py>(
    inputs: Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyDict>)> {
    let py: Python<'py> = inputs.py();
    let enums: Bound<'py, PyDict> = PyDict::new(py);
    let constants: Bound<'py, PyDict> = PyDict::new(py);
    index_declarations(&native_field(&inputs, "enum_files")?, "enum", &enums)?;
    index_declarations(
        &native_field(&inputs, "constant_files")?,
        "constant",
        &constants,
    )?;
    Ok((enums, constants))
}
