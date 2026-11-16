//! The persistent scope index cache: fingerprint, verified read and atomic write.

use crate::scopes::main::index::project_fingerprint::project_fingerprint;
use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAnyMethods, PyModule, PyResult, Python};
use pyo3::pyfunction;

use sqlbuild_python_base::records::main::fields::field::field as native_field;

const METADATA_MODULE: &str = "importlib.metadata";
const DISTRIBUTION_NAME: &str = "sqlbuild";
const UNKNOWN_VERSION: &str = "0+unknown";

/// `importlib.metadata.version("sqlbuild")`, or `0+unknown` when it is not installed.
pub(crate) fn sqlbuild_version(py: Python<'_>) -> PyResult<String> {
    let metadata: Bound<'_, PyModule> = py.import(METADATA_MODULE)?;
    match native_field(&metadata, "version")?.call1((DISTRIBUTION_NAME,)) {
        Ok(version) => version.extract(),
        Err(error) if error.is_instance(py, &native_field(&metadata, "PackageNotFoundError")?) => {
            Ok(UNKNOWN_VERSION.to_owned())
        }
        Err(error) => Err(error),
    }
}

/// Hash the canonical scope-relevant authored inputs, or `None` when they cannot be read.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub(crate) fn scope_index_fingerprint(
    py: Python<'_>,
    project_dir: PathBuf,
) -> PyResult<Option<String>> {
    project_fingerprint(py, project_dir)
}
