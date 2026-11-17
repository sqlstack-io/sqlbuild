//! Register the audits functions and classes.

use crate::planner_strategy::_helpers::audits::reads_outside_target_lineage;
use crate::planner_strategy::_helpers::audits::resolve_attachment_kind;
use crate::planner_strategy::_helpers::audits::resolve_effective_run_scope;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the audits functions and classes.
pub fn register_audits(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_attachment_kind, module)?)?;
    module.add_function(wrap_pyfunction!(reads_outside_target_lineage, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_effective_run_scope, module)?)
}
