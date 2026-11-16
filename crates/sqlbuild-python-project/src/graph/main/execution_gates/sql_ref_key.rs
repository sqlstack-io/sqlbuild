//! The compiled graph key a typed model, source or seed reference names.

use crate::graph::_helpers::execution_gates;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The compiled graph key a typed model, source or seed reference names.
#[inline]
pub fn sql_ref_key<'py>(
    py: Python<'py>,
    reference: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    execution_gates::sql_ref_key(py, reference)
}
