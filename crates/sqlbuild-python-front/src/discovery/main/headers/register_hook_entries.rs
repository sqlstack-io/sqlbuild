//! Register the hook entries functions and classes.

use crate::discovery::_helpers::headers::hook_entries;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the hook entries functions and classes.
#[inline]
pub fn register_hook_entries(module: &Bound<'_, PyModule>) -> PyResult<()> {
    hook_entries::register(module)
}
