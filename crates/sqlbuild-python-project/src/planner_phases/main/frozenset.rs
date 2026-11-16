//! A frozenset of the items.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyFrozenSet;

/// A frozenset of the items.
pub fn frozenset<'py>(py: Python<'py>, items: &[Bound<'py, PyAny>]) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyFrozenSet::new(py, items)?.into_any())
}
