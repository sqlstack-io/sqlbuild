//! Register the module digests functions and classes.

use crate::macro_bridge::_helpers::module_digests;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the module digests functions and classes.
#[inline]
pub fn register_module_digests(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module_digests::register(module)
}
