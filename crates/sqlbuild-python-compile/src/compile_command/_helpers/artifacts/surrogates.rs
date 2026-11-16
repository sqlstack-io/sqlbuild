//! Python text with lone surrogates carried through Rust formatting and handed back unchanged.

use std::cell::Cell;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBytes, PyString, PyStringMethods};

/// Lone surrogates travel as Supplementary Private Use Area-A characters, one per surrogate.
const CARRIER_BASE: u32 = 0xF_0000;
const SURROGATE_BASE: u32 = 0xD800;
const SURROGATE_COUNT: u32 = 0x800;
const UTF16: &str = "utf-16-le";
const SURROGATE_PASS: &str = "surrogatepass";

/// Reads Python text for Rust formatting, remembering whether any lone surrogate was carried.
#[derive(Default)]
pub(crate) struct SurrogateText {
    carried: Cell<bool>,
}

impl SurrogateText {
    /// `str(value)` as Rust text; each lone surrogate becomes its carrier character.
    pub(crate) fn text(&self, value: &Bound<'_, PyAny>) -> PyResult<String> {
        let text: Bound<'_, PyString> = value.str()?;
        if let Ok(plain) = text.to_str() {
            return Ok(plain.to_owned());
        }
        self.carried.set(true);
        let encoded: Vec<u8> = text
            .call_method1("encode", (UTF16, SURROGATE_PASS))?
            .extract()?;
        let units = encoded
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair));
        Ok(char::decode_utf16(units)
            .map(|decoded| {
                decoded.unwrap_or_else(|error| {
                    char::from_u32(
                        CARRIER_BASE + u32::from(error.unpaired_surrogate()) - SURROGATE_BASE,
                    )
                    .unwrap_or(char::REPLACEMENT_CHARACTER)
                })
            })
            .collect())
    }

    /// `str(value)`, or `None` for `None`.
    pub(crate) fn optional(&self, value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
        if value.is_none() {
            return Ok(None);
        }
        self.text(value).map(Some)
    }

    /// Formatted text as a Python string, with every carried surrogate restored.
    pub(crate) fn python<'py>(
        &self,
        py: Python<'py>,
        text: &str,
    ) -> PyResult<Bound<'py, PyString>> {
        if !self.carried.get() {
            return Ok(PyString::new(py, text));
        }
        let mut encoded: Vec<u8> = Vec::with_capacity(text.len() * 2);
        for character in text.chars() {
            let code: u32 = u32::from(character);
            if (CARRIER_BASE..CARRIER_BASE + SURROGATE_COUNT).contains(&code) {
                let unit: u16 =
                    u16::try_from(code - CARRIER_BASE + SURROGATE_BASE).unwrap_or(u16::MAX);
                encoded.extend(unit.to_le_bytes());
                continue;
            }
            let mut units: [u16; 2] = [0; 2];
            for unit in character.encode_utf16(&mut units) {
                encoded.extend(unit.to_le_bytes());
            }
        }
        Ok(PyBytes::new(py, &encoded)
            .call_method1("decode", (UTF16, SURROGATE_PASS))?
            .downcast_into()?)
    }
}
