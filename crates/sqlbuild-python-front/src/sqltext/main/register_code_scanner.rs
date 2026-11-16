//! Register the code scanner functions and classes.

use crate::sqltext::models::SqlCodeScanner;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the code scanner functions and classes.
pub fn register_code_scanner(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SqlCodeScanner>()?;
    Ok(())
}
