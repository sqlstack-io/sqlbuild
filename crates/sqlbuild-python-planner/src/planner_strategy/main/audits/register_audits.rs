//! Register the audits functions and classes.

use crate::planner_strategy::_helpers::audits;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the audits functions and classes.
#[inline]
pub fn register_audits(module: &Bound<'_, PyModule>) -> PyResult<()> {
    audits::register(module)
}
