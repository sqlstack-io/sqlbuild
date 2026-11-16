//! Whether two type strings are semantically equal under `dialect` (a `TypeDialect`, name or `None`).

use crate::type_system::_helpers::equality;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Whether two type strings are semantically equal under `dialect` (a `TypeDialect`, name or `None`).
#[inline]
pub fn types_equal(dialect: &Bound<'_, PyAny>, left: &str, right: &str) -> PyResult<bool> {
    equality::types_equal(dialect, left, right)
}
