//! Register the nullability functions and classes.

use crate::type_system::_helpers::nullability::infer_conditional_result_nullability;
use crate::type_system::_helpers::nullability::infer_first_arg_nullability;
use crate::type_system::_helpers::nullability::infer_safe_cast_nullability;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the nullability functions and classes.
pub fn register_nullability(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(infer_first_arg_nullability, module)?)?;
    module.add_function(wrap_pyfunction!(
        infer_conditional_result_nullability,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(infer_safe_cast_nullability, module)?)?;
    Ok(())
}
