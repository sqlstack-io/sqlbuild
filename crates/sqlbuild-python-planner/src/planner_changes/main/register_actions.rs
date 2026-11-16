//! Register the actions functions and classes.

use crate::planner_changes::main::replaces_incremental_relation::replaces_incremental_relation;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the actions functions and classes.
pub fn register_actions(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(replaces_incremental_relation, module)?)
}
