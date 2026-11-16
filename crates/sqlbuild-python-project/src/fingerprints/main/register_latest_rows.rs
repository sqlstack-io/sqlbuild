//! Register the latest rows functions and classes.

use crate::fingerprints::main::select_latest_fingerprints::select_latest_fingerprints;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the latest rows functions and classes.
pub fn register_latest_rows(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(select_latest_fingerprints, module)?)?;
    Ok(())
}
