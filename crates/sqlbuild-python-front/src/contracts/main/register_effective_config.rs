//! Register the effective config functions and classes.

use crate::contracts::_helpers::effective_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the effective config functions and classes.
#[inline]
pub fn register_effective_config(module: &Bound<'_, PyModule>) -> PyResult<()> {
    effective_config::register(module)
}
