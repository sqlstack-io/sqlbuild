//! Register the scope validation functions and classes.

use crate::scopes::_helpers::checks::scope_validation::scope_placement_validated_index;
use crate::scopes::_helpers::checks::scope_validation::scope_relocated_declarations;
use crate::scopes::main::checks::validate_scope_index::validate_scope_index;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the scope validation functions and classes.
pub fn register_scope_validation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(validate_scope_index, module)?)?;
    module.add_function(wrap_pyfunction!(scope_placement_validated_index, module)?)?;
    module.add_function(wrap_pyfunction!(scope_relocated_declarations, module)?)?;
    Ok(())
}
