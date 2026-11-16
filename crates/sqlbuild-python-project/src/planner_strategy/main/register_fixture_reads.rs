//! Register the fixture reads functions and classes.

use crate::planner_strategy::_helpers::fixtures::fixture_reads::fixture_read_cache_info;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the fixture reads functions and classes.
pub fn register_fixture_reads(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(fixture_read_cache_info, module)?)
}
