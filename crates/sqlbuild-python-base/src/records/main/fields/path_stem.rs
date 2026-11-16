//! `path.stem` of a `pathlib` path, spelled natively from its text.

use crate::records::_helpers::native_classes::fields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `path.stem` of a `pathlib` path, spelled natively from its text.
#[inline]
pub fn path_stem(path: &Bound<'_, PyAny>) -> PyResult<String> {
    fields::path_stem(path)
}
