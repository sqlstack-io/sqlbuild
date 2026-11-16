//! Register the declaration values functions and classes.

use crate::render::_helpers::declarations::declaration_values::build_native_declaration_contexts;
use crate::render::_helpers::declarations::declaration_values::resolve_declaration_context;
use crate::render::main::declarations::render_enum_member_value::render_enum_member_value;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the declaration values functions and classes.
pub fn register_declaration_values(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_native_declaration_contexts, module)?)?;
    module.add_function(wrap_pyfunction!(render_enum_member_value, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_declaration_context, module)?)?;
    Ok(())
}
