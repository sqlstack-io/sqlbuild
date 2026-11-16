//! Register the scope visibility functions and classes.

use crate::scopes::_helpers::checks::scope_visibility::resolve_scope_declaration_visibility;
use crate::scopes::_helpers::checks::scope_visibility::resolve_scope_visibility;
use crate::scopes::_helpers::checks::scope_visibility::scope_query_target;
use crate::scopes::_helpers::checks::scope_visibility::scope_resolve_path_visibility;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the scope visibility functions and classes.
pub fn register_scope_visibility(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(scope_query_target, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_scope_visibility, module)?)?;
    module.add_function(wrap_pyfunction!(
        resolve_scope_declaration_visibility,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(scope_resolve_path_visibility, module)?)?;
    Ok(())
}
