//! Whether native discovery reproduces the string semantics of this Python and Unicode version.

use crate::discovery::_helpers::project::project_files::parse_model_contents;
use crate::discovery::_helpers::snapshots::native_classes::NativeProjectTree;
use crate::discovery::models::NativeModelFile;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Whether native discovery reproduces the string semantics of this Python and Unicode version.
pub fn register_project_files(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeProjectTree>()?;
    module.add_class::<NativeModelFile>()?;
    module.add_function(wrap_pyfunction!(parse_model_contents, module)?)?;
    Ok(())
}
