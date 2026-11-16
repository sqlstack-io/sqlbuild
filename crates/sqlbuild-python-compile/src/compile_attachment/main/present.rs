//! `value`, or `None` when it is Python `None`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods};

/// `value`, or `None` when it is Python `None`.
pub fn present(value: Bound<'_, PyAny>) -> Option<Bound<'_, PyAny>> {
    (!value.is_none()).then_some(value)
}
