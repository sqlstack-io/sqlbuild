//! `(years, months, days, hours, minutes, seconds)` of a duration like `1y6mo` or `30d`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyInt, PyTuple};
use sqlbuild_cursor::algebra::main::parse_duration::parse_duration;
use sqlbuild_cursor::algebra::models::DurationDigits;

/// `(years, months, days, hours, minutes, seconds)` of a duration like `1y6mo` or `30d`.
#[pyfunction]
#[pyo3(signature = (value))]
pub fn parse_cursor_duration<'py>(
    py: Python<'py>,
    value: &str,
) -> PyResult<Option<Bound<'py, PyTuple>>> {
    let Some(digits): Option<DurationDigits> = parse_duration(value) else {
        return Ok(None);
    };
    let int = py.get_type::<PyInt>();
    let parts: Vec<Bound<'py, PyAny>> = [
        digits.years,
        digits.months,
        digits.days,
        digits.hours,
        digits.minutes,
        digits.seconds,
    ]
    .iter()
    .map(|text| int.call1((text.as_str(),)))
    .collect::<PyResult<_>>()?;
    PyTuple::new(py, parts).map(Some)
}
