//! Public records of manifest.

use pyo3::exceptions::PyRecursionError;
use pyo3::exceptions::PyTypeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyBytes;
use sqlbuild_analysis::manifest::errors::ManifestEncodeError;
use sqlbuild_core::json::errors::JsonEmitError;

/// A built manifest: its `manifest.json` bytes, or the error `json.dumps` raised encoding it.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeManifest {
    pub(crate) contents: Result<Vec<u8>, ManifestEncodeError>,
}

#[pymethods]
impl NativeManifest {
    /// The bytes `json.dumps(manifest, indent=2) + "\n"` encoded, raising what it raised.
    pub(crate) fn contents<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        match &self.contents {
            Ok(bytes) => Ok(PyBytes::new(py, bytes)),
            Err(ManifestEncodeError::Unencodable(message)) => {
                Err(PyTypeError::new_err(message.clone()))
            }
            Err(ManifestEncodeError::Emit(JsonEmitError::NestingTooDeep)) => Err(
                PyRecursionError::new_err("maximum recursion depth exceeded"),
            ),
            Err(ManifestEncodeError::Emit(error)) => {
                Err(PyValueError::new_err(format!("{error:?}")))
            }
        }
    }
}
