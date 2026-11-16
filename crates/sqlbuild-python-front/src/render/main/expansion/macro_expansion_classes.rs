//! The classes, enums and helpers the native expansion builds facts and errors from, built once.

use crate::render::_helpers::expansion::macro_expansion_result::EXPANSION_CLASSES;
use crate::render::_helpers::expansion::macro_expansion_result::built_classes;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::PyDict;

/// The classes, enums and helpers the native expansion builds facts and errors from, built once.
pub fn macro_expansion_classes(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    EXPANSION_CLASSES
        .get_or_try_init(py, || built_classes(py).map(Bound::unbind))
        .map(|classes| classes.bind(py).clone())
}
