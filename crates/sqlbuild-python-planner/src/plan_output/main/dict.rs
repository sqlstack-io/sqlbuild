//! A dictionary of the entries, in order.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};

/// A dictionary of the entries, in order.
pub fn dict<'py>(
    py: Python<'py>,
    entries: Vec<(&str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyDict>> {
    let built: Bound<'py, PyDict> = PyDict::new(py);
    for (key, value) in entries {
        built.set_item(key, value)?;
    }
    Ok(built)
}
