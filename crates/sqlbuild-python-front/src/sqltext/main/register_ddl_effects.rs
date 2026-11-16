//! Register the ddl effects functions and classes.

use crate::sqltext::_helpers::ddl_effects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the ddl effects functions and classes.
#[inline]
pub fn register_ddl_effects(module: &Bound<'_, PyModule>) -> PyResult<()> {
    ddl_effects::register(module)
}
