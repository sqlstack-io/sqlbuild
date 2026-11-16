//! Register the declaration files functions and classes.

use crate::discovery::_helpers::snapshots::native_classes::NativeDiscoverySession;
use crate::discovery::_helpers::sources::declaration_files::parse_declaration_contents;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the declaration files functions and classes.
pub fn register_declaration_files(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeDiscoverySession>()?;
    module.add_function(wrap_pyfunction!(parse_declaration_contents, module)?)?;
    Ok(())
}
