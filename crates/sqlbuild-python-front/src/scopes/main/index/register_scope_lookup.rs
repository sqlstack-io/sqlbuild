//! Register the scope lookup functions and classes.

use crate::scopes::_helpers::index::scope_lookup::NativeScopeLookup;
use crate::scopes::_helpers::index::scope_lookup::scope_normalize_path;
use crate::scopes::main::index::build_scope_lookup::build_scope_lookup;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the scope lookup functions and classes.
pub fn register_scope_lookup(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeScopeLookup>()?;
    module.add_function(wrap_pyfunction!(build_scope_lookup, module)?)?;
    module.add_function(wrap_pyfunction!(scope_normalize_path, module)?)?;
    Ok(())
}
