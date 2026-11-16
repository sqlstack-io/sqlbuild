//! Comparable bounds, presuming a naive side UTC when the other is aware.

use crate::plan_output::_helpers::display::as_datetime;
use crate::plan_output::_helpers::display::presume_utc;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Comparable bounds, presuming a naive side UTC when the other is aware.
#[pyfunction]
#[pyo3(signature = (*, start, end))]
pub fn aligned_timestamp_bounds<'py>(
    start: &Bound<'py, PyAny>,
    end: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = start.py();
    let mut start_at: Bound<'py, PyAny> = as_datetime(start)?;
    let mut end_at: Bound<'py, PyAny> = as_datetime(end)?;
    if native_field(&start_at, "tzinfo")?.is_none() != native_field(&end_at, "tzinfo")?.is_none() {
        start_at = presume_utc(start_at)?;
        end_at = presume_utc(end_at)?;
    }
    PyTuple::new(py, [start_at, end_at])
}
