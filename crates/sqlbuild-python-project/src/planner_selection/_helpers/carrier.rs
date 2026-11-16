//! Carry lone surrogates through the UTF-8 selector grammar as unused private-use characters.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;

pub(crate) const PRIVATE_USE: std::ops::Range<u32> = 0xF0000..0xFFFFE;

/// Python strings for each item of an iterable of `str`.
pub(crate) fn py_strings<'py>(values: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyString>>> {
    values
        .try_iter()?
        .map(|value| Ok(value?.downcast_into::<PyString>()?))
        .collect()
}

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
