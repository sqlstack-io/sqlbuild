//! Register the declaration indexes functions and classes.

use crate::render::main::declarations::public_declaration_indexes::public_declaration_indexes;
use crate::render::main::declarations::public_model_schema_index::public_model_schema_index;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the declaration indexes functions and classes.
pub fn register_declaration_indexes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(public_declaration_indexes, module)?)?;
    module.add_function(wrap_pyfunction!(public_model_schema_index, module)?)
}
