//! Register the projections functions and classes.

use crate::planner_migrations::_helpers::projections::projections;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the projections functions and classes.
#[inline]
pub fn register_projections(module: &Bound<'_, PyModule>) -> PyResult<()> {
    projections::register(module)
}
