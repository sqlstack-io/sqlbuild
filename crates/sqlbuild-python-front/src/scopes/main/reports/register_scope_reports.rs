//! Register the scope reports functions and classes.

use crate::scopes::_helpers::reports::scope_reports::browse_scope_folders_binding;
use crate::scopes::_helpers::reports::scope_reports::explain_scope_declaration_binding;
use crate::scopes::_helpers::reports::scope_reports::list_scope_declarations_binding;
use crate::scopes::_helpers::reports::scope_reports::metadata_projection;
use crate::scopes::_helpers::reports::scope_reports::query_scope_report_binding;
use crate::scopes::main::reports::scope_move_preview::scope_move_preview;
use crate::scopes::main::reports::serialize_scope_report::serialize_scope_report;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the scope reports functions and classes.
pub fn register_scope_reports(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(query_scope_report_binding, module)?)?;
    module.add_function(wrap_pyfunction!(explain_scope_declaration_binding, module)?)?;
    module.add_function(wrap_pyfunction!(browse_scope_folders_binding, module)?)?;
    module.add_function(wrap_pyfunction!(list_scope_declarations_binding, module)?)?;
    module.add_function(wrap_pyfunction!(scope_move_preview, module)?)?;
    module.add_function(wrap_pyfunction!(serialize_scope_report, module)?)?;
    module.add_function(wrap_pyfunction!(metadata_projection, module)?)?;
    Ok(())
}
