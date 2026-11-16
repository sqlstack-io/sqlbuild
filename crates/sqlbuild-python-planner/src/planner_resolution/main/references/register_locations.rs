//! Register the locations functions and classes.

use crate::planner_resolution::main::references::build_function_locations::build_function_locations;
use crate::planner_resolution::main::references::build_model_locations::build_model_locations;
use crate::planner_resolution::main::references::build_seed_locations::build_seed_locations;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the locations functions and classes.
pub fn register_locations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_model_locations, module)?)?;
    module.add_function(wrap_pyfunction!(build_seed_locations, module)?)?;
    module.add_function(wrap_pyfunction!(build_function_locations, module)?)
}
