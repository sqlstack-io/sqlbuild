//! Carry lone surrogates through the UTF-8 selector grammar as unused private-use characters.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};

pub(crate) const PRIVATE_USE: std::ops::Range<u32> = 0xF0000..0xFFFFE;

pub(crate) fn ordinal(character: &Bound<'_, PyAny>) -> PyResult<u32> {
    let code: u32 = character
        .py()
        .import("builtins")?
        .getattr("ord")?
        .call1((character,))?
        .extract()?;
    Ok(code)
}

pub(crate) fn surrogate_text(py: Python<'_>, code: u32) -> PyResult<Bound<'_, PyAny>> {
    py.import("builtins")?.getattr("chr")?.call1((code,))
}
