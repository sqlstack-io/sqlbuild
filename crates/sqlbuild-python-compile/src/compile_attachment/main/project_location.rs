//! A project-level location: the target's value, then `[defaults]`, then the first set fallback.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};

/// A project-level location: the target's value, then `[defaults]`, then the first set fallback.
pub fn project_location<'py>(
    (target, default): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    fallbacks: Vec<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target.py();
    for value in [target, default].into_iter().chain(fallbacks) {
        if value.is_truthy()? {
            return Ok(value);
        }
    }
    Ok(py.None().into_bound(py))
}
