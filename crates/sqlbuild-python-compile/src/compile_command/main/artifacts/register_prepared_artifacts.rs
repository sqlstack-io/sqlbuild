//! Register the prepared artifacts functions and classes.

use crate::compile_command::_helpers::artifacts::prepared_artifacts::PreparedCompileArtifacts;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the prepared artifacts functions and classes.
pub fn register_prepared_artifacts(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PreparedCompileArtifacts>()
}
