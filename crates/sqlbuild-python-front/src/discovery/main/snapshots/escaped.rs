//! Python's `_escaped`: undecodable name bytes as `\xNN`, other lone surrogates as `\uXXXX`.

use crate::discovery::_helpers::snapshots::snapshot::SURROGATE_PASS;
use crate::discovery::_helpers::snapshots::snapshot::UTF16_ENCODING;
use crate::discovery::_helpers::snapshots::snapshot::escaped_unit;
use pyo3::prelude::{Bound, PyAnyMethods, PyResult};
use pyo3::types::PyString;

/// Python's `_escaped`: undecodable name bytes as `\xNN`, other lone surrogates as `\uXXXX`.
pub fn escaped(text: &Bound<'_, PyString>) -> PyResult<String> {
    let encoded: Vec<u8> = text
        .call_method1("encode", (UTF16_ENCODING, SURROGATE_PASS))?
        .extract()?;
    let units = encoded
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair));
    Ok(char::decode_utf16(units)
        .map(|decoded| match decoded {
            Ok(character) => character.to_string(),
            Err(error) => escaped_unit(error.unpaired_surrogate()),
        })
        .collect())
}
