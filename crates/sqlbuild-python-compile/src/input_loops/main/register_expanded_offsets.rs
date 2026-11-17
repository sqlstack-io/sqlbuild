//! Register the expanded offsets functions and classes.

use crate::input_loops::_helpers::resources::expanded_offsets::map_expanded_offset;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the expanded offsets functions and classes.
pub fn register_expanded_offsets(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(map_expanded_offset, module)?)
}
