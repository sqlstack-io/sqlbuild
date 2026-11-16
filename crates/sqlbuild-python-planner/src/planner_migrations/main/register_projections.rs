//! Register the projections functions and classes.

use crate::planner_migrations::_helpers::projections::query_shapes::parse_query_shape;
use crate::planner_migrations::_helpers::projections::query_shapes::renames_explain_change;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the projections functions and classes.
pub fn register_projections(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_query_shape, module)?)?;
    module.add_function(wrap_pyfunction!(renames_explain_change, module)?)
}
