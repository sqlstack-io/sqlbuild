//! Register the model selectors functions and classes.

use crate::planner_selection::_helpers::model_selectors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the model selectors functions and classes.
#[inline]
pub fn register_model_selectors(module: &Bound<'_, PyModule>) -> PyResult<()> {
    model_selectors::register(module)
}
