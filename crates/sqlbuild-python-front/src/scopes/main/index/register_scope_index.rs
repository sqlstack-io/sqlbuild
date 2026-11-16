//! Register the scope index functions and classes.

use crate::scopes::_helpers::index::scope_index::build_scope_index_binding;
use crate::scopes::main::index::open_native_scope_index::open_native_scope_index;
use crate::scopes::models::NativeScopeIndex;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the scope index functions and classes.
pub fn register_scope_index(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeScopeIndex>()?;
    module.add_function(wrap_pyfunction!(open_native_scope_index, module)?)?;
    module.add_function(wrap_pyfunction!(build_scope_index_binding, module)?)?;
    Ok(())
}
