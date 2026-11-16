//! Register the planning functions and classes.

use crate::freshness::_helpers::planning::build_direct_source_freshness_planning_result;
use crate::freshness::_helpers::planning::build_direct_source_freshness_propagation_result;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the planning functions and classes.
pub fn register_planning(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_direct_source_freshness_planning_result,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        build_direct_source_freshness_propagation_result,
        module
    )?)?;
    Ok(())
}
