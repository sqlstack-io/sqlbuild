//! Register the model entries functions and classes.

use crate::planner_entries::_helpers::entries::model_entries;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the model entries functions and classes.
#[inline]
pub fn register_model_entries(module: &Bound<'_, PyModule>) -> PyResult<()> {
    model_entries::register(module)
}
