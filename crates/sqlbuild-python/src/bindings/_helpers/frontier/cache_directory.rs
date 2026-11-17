//! The project-local compiler store root.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::compile_command::constants::TARGET_DIRECTORY_NAME;
use crate::bindings::_helpers::frontier::constants::CACHE_DIRECTORY_NAME;
use crate::bindings::_helpers::frontier::constants::COMPILER_CACHE_DIRECTORY_NAME;
use crate::bindings::_helpers::frontier::constants::NATIVE_CACHE_NAMESPACE_SUFFIX;

/// `project_dir / target / cache / compiler<suffix>`, keeping the path type of `project_dir`.
pub(crate) fn cache_directory<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let store: String = format!("{COMPILER_CACHE_DIRECTORY_NAME}{NATIVE_CACHE_NAMESPACE_SUFFIX}");
    project_dir
        .div(TARGET_DIRECTORY_NAME)?
        .div(CACHE_DIRECTORY_NAME)?
        .div(store)
}

/// Return the project's compiler store root.
#[pyfunction]
fn compiler_cache_directory<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    cache_directory(project_dir)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compiler_cache_directory, module)?)
}
