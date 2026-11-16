//! Register the model entries functions and classes.

use crate::planner_entries::_helpers::entries::model_entries::scope_overlaps;
use crate::planner_entries::main::entries::plan_model::plan_model;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the model entries functions and classes.
pub fn register_model_entries(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(plan_model, module)?)?;
    module.add_function(wrap_pyfunction!(scope_overlaps, module)?)
}
