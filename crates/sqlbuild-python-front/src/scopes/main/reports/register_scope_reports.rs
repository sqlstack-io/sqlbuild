//! Register the scope reports functions and classes.

use crate::scopes::_helpers::reports::scope_reports;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the scope reports functions and classes.
#[inline]
pub fn register_scope_reports(module: &Bound<'_, PyModule>) -> PyResult<()> {
    scope_reports::register(module)
}
