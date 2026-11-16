//! Every item of an iterable.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// Every item of an iterable.
pub fn items<'py>(value: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    value.try_iter()?.collect()
}
