//! The sorted texts of a set of strings.

use crate::sqltext::_helpers::lexical_syntax;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The sorted texts of a set of strings.
#[inline]
pub fn sorted_texts(values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    lexical_syntax::sorted_texts(values)
}
