//! The strings of any Python iterable, in iteration order.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// The strings of any Python iterable, in iteration order.
pub fn strings(values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    values
        .try_iter()?
        .map(|value| value?.extract::<String>())
        .collect()
}
