//! Register the views functions and classes.

use crate::graph::_helpers::project::views;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the views functions and classes.
#[inline]
pub fn register_views(module: &Bound<'_, PyModule>) -> PyResult<()> {
    views::register(module)
}
