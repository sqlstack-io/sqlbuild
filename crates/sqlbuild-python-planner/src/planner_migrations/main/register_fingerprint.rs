//! Register the fingerprint functions and classes.

use crate::planner_migrations::_helpers::projections::fingerprint::build_migration_fingerprint;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the fingerprint functions and classes.
pub fn register_fingerprint(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_migration_fingerprint, module)?)?;
    Ok(())
}
