//! Register the audit progress functions and classes.

use crate::build_progress::models::AuditProgressReporter;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the audit progress functions and classes.
pub fn register_audit_progress(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<AuditProgressReporter>()
}
