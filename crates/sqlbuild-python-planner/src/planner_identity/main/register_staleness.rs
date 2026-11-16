//! Register the staleness functions and classes.

use crate::planner_identity::_helpers::staleness::classify_selection_staleness_warnings;
use crate::planner_identity::main::build_stale_out_of_selection_warnings::build_stale_out_of_selection_warnings;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the staleness functions and classes.
pub fn register_staleness(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_stale_out_of_selection_warnings,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        classify_selection_staleness_warnings,
        module
    )?)
}
