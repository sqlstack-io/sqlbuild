//! Register the builder functions and classes.

use crate::manifest::main::build_manifest::build_manifest;
use crate::manifest::models::NativeManifest;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the builder functions and classes.
pub fn register_builder(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_manifest, module)?)?;
    module.add_class::<NativeManifest>()
}
