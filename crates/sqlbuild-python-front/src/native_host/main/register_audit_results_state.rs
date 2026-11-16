//! Register the audit results state functions and classes.

use crate::native_host::_helpers::audit_results_state::build_audit_results_prune_sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the audit results state functions and classes.
pub fn register_audit_results_state(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_audit_results_prune_sql, module)?)
}
