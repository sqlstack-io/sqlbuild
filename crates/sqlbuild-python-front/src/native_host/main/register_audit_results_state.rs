//! Register the audit results state functions and classes.

use crate::native_host::_helpers::audit_results_state;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the audit results state functions and classes.
#[inline]
pub fn register_audit_results_state(module: &Bound<'_, PyModule>) -> PyResult<()> {
    audit_results_state::register(module)
}
