//! `path.as_posix()` of a `pathlib` path, its text with the platform separator spelled `/`.

use crate::records::_helpers::native_classes::fields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `path.as_posix()` of a `pathlib` path, its text with the platform separator spelled `/`.
#[inline]
pub fn path_posix_text(path: &Bound<'_, PyAny>) -> PyResult<String> {
    fields::path_posix_text(path)
}
