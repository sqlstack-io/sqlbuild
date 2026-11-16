//! Register the compile usages functions and classes.

use crate::scopes::_helpers::relationships::compile_usages;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the compile usages functions and classes.
#[inline]
pub fn register_compile_usages(module: &Bound<'_, PyModule>) -> PyResult<()> {
    compile_usages::register(module)
}
