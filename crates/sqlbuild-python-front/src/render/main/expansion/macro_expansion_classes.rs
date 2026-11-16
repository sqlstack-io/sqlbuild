//! The classes, enums and helpers the native expansion builds facts and errors from, built once.

use crate::render::_helpers::expansion::macro_expansion_result;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// The classes, enums and helpers the native expansion builds facts and errors from, built once.
#[inline]
pub fn macro_expansion_classes(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    macro_expansion_result::macro_expansion_classes(py)
}
