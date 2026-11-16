//! Register the unicode environment functions and classes.

use crate::model_config::_helpers::model_nodes::unicode_environment;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the unicode environment functions and classes.
#[inline]
pub fn register_unicode_environment(module: &Bound<'_, PyModule>) -> PyResult<()> {
    unicode_environment::register(module)
}
