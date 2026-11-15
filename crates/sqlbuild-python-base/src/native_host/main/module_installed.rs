//! Whether this interpreter can import the top-level module `name`.

use crate::native_host::_helpers::host;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Whether this interpreter can import the top-level module `name`.
#[inline]
pub fn module_installed(py: Python<'_>, name: &str) -> PyResult<bool> {
    host::module_installed(py, name)
}
