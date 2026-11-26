//! A Python `str` as UTF-16 code units, which keeps lone surrogates `to_str` rejects.

use pyo3::prelude::{Bound, PyAnyMethods, PyResult};
use pyo3::types::{PyBytes, PyBytesMethods, PyString};

/// The UTF-16 code units of `text`, lone surrogates included.
pub(crate) fn utf16_units(text: &Bound<'_, PyString>) -> PyResult<Vec<u16>> {
    let encoded: Bound<'_, PyBytes> = text
        .call_method1("encode", ("utf-16-le", "surrogatepass"))?
        .cast_into::<PyBytes>()?;
    Ok(encoded
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect())
}
