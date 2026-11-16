//! Register the fixture completion functions and classes.

use crate::planner_strategy::_helpers::fixtures::fixture_completion::build_relation_fixture_completion;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the fixture completion functions and classes.
pub fn register_fixture_completion(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_relation_fixture_completion, module)?)
}
