//! The UTF-16 code units of a Python `str`, lone surrogates included.

use crate::boundary::_helpers::text_units;
use pyo3::prelude::{Bound, PyResult};
use pyo3::types::PyString;

/// The UTF-16 code units of a Python `str`, lone surrogates included.
#[inline]
pub fn utf16_units(text: &Bound<'_, PyString>) -> PyResult<Vec<u16>> {
    text_units::utf16_units(text)
}
