//! Register the cursor intrinsics functions and classes.

use crate::attachments::_helpers::cursor_intrinsics::cursor_intrinsics_analysis_sql;
use crate::attachments::_helpers::cursor_intrinsics::validated_model_cursor_intrinsics;
use crate::attachments::main::resolve_cursor_intrinsics::resolve_cursor_intrinsics;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the cursor intrinsics functions and classes.
pub fn register_cursor_intrinsics(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(validated_model_cursor_intrinsics, module)?)?;
    module.add_function(wrap_pyfunction!(cursor_intrinsics_analysis_sql, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_cursor_intrinsics, module)?)?;
    Ok(())
}
