//! `project_dir / target / cache / compiler<suffix>`, keeping the path type of `project_dir`.

use crate::compile_command::constants::TARGET_DIRECTORY_NAME;
use crate::frontier::constants::CACHE_DIRECTORY_NAME;
use crate::frontier::constants::COMPILER_CACHE_DIRECTORY_NAME;
use crate::frontier::constants::NATIVE_CACHE_NAMESPACE_SUFFIX;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// `project_dir / target / cache / compiler<suffix>`, keeping the path type of `project_dir`.
pub fn cache_directory<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let store: String = format!("{COMPILER_CACHE_DIRECTORY_NAME}{NATIVE_CACHE_NAMESPACE_SUFFIX}");
    project_dir
        .div(TARGET_DIRECTORY_NAME)?
        .div(CACHE_DIRECTORY_NAME)?
        .div(store)
}
