//! Register the identity functions and classes.

use crate::auditing::_helpers::identity::build_audit_gate_identity;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the identity functions and classes.
pub fn register_identity(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_audit_gate_identity, module)?)?;
    Ok(())
}
