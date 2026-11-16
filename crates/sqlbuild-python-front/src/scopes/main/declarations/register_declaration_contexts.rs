//! Register the declaration contexts functions and classes.

use crate::scopes::models::NativeDeclarationContexts;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the declaration contexts functions and classes.
pub fn register_declaration_contexts(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeDeclarationContexts>()?;
    Ok(())
}
