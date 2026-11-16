//! Row fingerprint.

use crate::fingerprints::_helpers::latest_rows;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Row fingerprint.
#[inline]
pub fn row_fingerprint<'py>(
    fingerprint_class: &Bound<'py, PyAny>,
    from_iso: &Bound<'py, PyAny>,
    row: &Bound<'py, PyAny>,
    qualified_name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    latest_rows::row_fingerprint(fingerprint_class, from_iso, row, qualified_name)
}
