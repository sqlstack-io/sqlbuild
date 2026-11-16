//! Register the hashing functions and classes.

use crate::planner_identity::main::model_definition_hash::model_definition_hash;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the hashing functions and classes.
pub fn register_hashing(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(model_definition_hash, module)?)
}
