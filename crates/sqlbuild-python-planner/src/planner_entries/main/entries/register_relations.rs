//! Register the relations functions and classes.

use crate::planner_entries::_helpers::entries::relations::build_planner_relations_context;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the relations functions and classes.
pub fn register_relations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_planner_relations_context, module)?)
}
