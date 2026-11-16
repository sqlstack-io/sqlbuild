//! Register the hook entries functions and classes.

use crate::discovery::_helpers::headers::hook_entries::serialize_hook_entries;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the hook entries functions and classes.
pub fn register_hook_entries(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(serialize_hook_entries, module)?)?;
    Ok(())
}
