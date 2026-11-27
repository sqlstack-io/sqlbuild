//! Command-line strings with lone surrogates (undecodable bytes) carried through Rust text.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBytes, PyBytesMethods, PyString, PyStringMethods};

/// Lone surrogates `U+D800..=U+DFFF` stand in Rust text as this private-use block, in order.
const PLACEHOLDER_START: u32 = 0x10_F800;
const SURROGATE_START: u32 = 0xD800;
const SURROGATE_COUNT: u32 = 0x800;
const SURROGATE_LEAD_BYTE: u8 = 0xED;
const SURROGATE_SECOND_MASK: u8 = 0xE0;
const SURROGATE_SECOND_BITS: u8 = 0xA0;
const PLACEHOLDER_LEAD_BYTES: [u8; 2] = [0xF4, 0x8F];
const CONTINUATION_MASK: u8 = 0x3F;
const CONTINUATION_BITS: u8 = 0x80;
const SURROGATE_PREFIX_BITS: u32 = 0xD000;
const ENCODING: &str = "utf-8";
const SURROGATE_HANDLER: &str = "surrogatepass";

/// A command-line string as Rust text, and whether it held lone surrogates.
pub(crate) fn rust_text(text: &Bound<'_, PyString>) -> PyResult<(String, bool)> {
    if let Ok(plain) = text.to_str() {
        return Ok((plain.to_owned(), false));
    }
    let encoded: Bound<'_, PyAny> = text.call_method1("encode", (ENCODING, SURROGATE_HANDLER))?;
    let bytes: &[u8] = encoded.cast::<PyBytes>()?.as_bytes();
    let mut mapped: Vec<u8> = Vec::with_capacity(bytes.len() + 1);
    let mut index: usize = 0;
    while index < bytes.len() {
        let window: &[u8] = &bytes[index..bytes.len().min(index + 3)];
        if let [SURROGATE_LEAD_BYTE, second, third] = window
            && second & SURROGATE_SECOND_MASK == SURROGATE_SECOND_BITS
        {
            let code: u32 = SURROGATE_PREFIX_BITS
                | (u32::from(second & CONTINUATION_MASK) << 6)
                | u32::from(third & CONTINUATION_MASK);
            let placeholder: char = char::from_u32(PLACEHOLDER_START + code - SURROGATE_START)
                .ok_or_else(|| PyValueError::new_err("invalid surrogate"))?;
            mapped.extend_from_slice(placeholder.encode_utf8(&mut [0; 4]).as_bytes());
            index += 3;
            continue;
        }
        mapped.push(bytes[index]);
        index += 1;
    }
    let mapped: String =
        String::from_utf8(mapped).map_err(|error| PyValueError::new_err(error.to_string()))?;
    if holds_placeholders(bytes) {
        return Err(PyValueError::new_err(
            "a command-line argument mixes undecodable bytes with U+10F800..U+10FFFF",
        ));
    }
    Ok((mapped, true))
}

fn holds_placeholders(bytes: &[u8]) -> bool {
    bytes.windows(3).any(|window| {
        window[..2] == PLACEHOLDER_LEAD_BYTES
            && window[2] & SURROGATE_SECOND_MASK == SURROGATE_SECOND_BITS
    })
}

/// Whether Rust text holds a character the surrogate placeholders use.
pub(crate) fn uses_placeholders(text: &str) -> bool {
    text.chars()
        .any(|character| placeholder_code(character).is_some())
}

fn placeholder_code(character: char) -> Option<u32> {
    let code: u32 = u32::from(character);
    (PLACEHOLDER_START..PLACEHOLDER_START + SURROGATE_COUNT)
        .contains(&code)
        .then(|| SURROGATE_START + code - PLACEHOLDER_START)
}

/// Rust text back as a Python `str`, restoring lone surrogates when `surrogates` is set.
pub(crate) fn python_text<'py>(
    py: Python<'py>,
    text: &str,
    surrogates: bool,
) -> PyResult<Bound<'py, PyAny>> {
    if !surrogates || !uses_placeholders(text) {
        return Ok(PyString::new(py, text).into_any());
    }
    let mut bytes: Vec<u8> = Vec::with_capacity(text.len());
    for character in text.chars() {
        match placeholder_code(character) {
            Some(code) => bytes.extend_from_slice(&[
                SURROGATE_LEAD_BYTE,
                CONTINUATION_BITS | ((code >> 6) as u8 & CONTINUATION_MASK),
                CONTINUATION_BITS | (code as u8 & CONTINUATION_MASK),
            ]),
            None => bytes.extend_from_slice(character.encode_utf8(&mut [0; 4]).as_bytes()),
        }
    }
    PyBytes::new(py, &bytes).call_method1("decode", (ENCODING, SURROGATE_HANDLER))
}
