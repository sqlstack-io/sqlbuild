//! Register the yaml entries functions and classes.

use crate::discovery::_helpers::sources::yaml_entries;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the yaml entries functions and classes.
#[inline]
pub fn register_yaml_entries(module: &Bound<'_, PyModule>) -> PyResult<()> {
    yaml_entries::register(module)
}
