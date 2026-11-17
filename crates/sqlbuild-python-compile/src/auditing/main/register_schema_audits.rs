//! Register the schema audits functions and classes.

use crate::auditing::_helpers::schema_audits::parse_audit_instances;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the schema audits functions and classes.
pub fn register_schema_audits(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_audit_instances, module)?)?;
    Ok(())
}
