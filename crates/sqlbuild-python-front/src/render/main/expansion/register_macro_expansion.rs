//! Register the macro expansion functions and classes.

use crate::render::models::NativeMacroExpansion;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the macro expansion functions and classes.
pub fn register_macro_expansion(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeMacroExpansion>()?;
    Ok(())
}
