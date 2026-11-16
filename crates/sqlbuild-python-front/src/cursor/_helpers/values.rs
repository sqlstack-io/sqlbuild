//! Python `date`, `datetime` and `timedelta` objects read into and built from cursor values.

use crate::cursor::constants::DATETIME_MODULE;
use crate::cursor::main::values::replaced::replaced;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_cursor::algebra::models::{CivilDate, LocalDateTime};

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const MICROS_PER_SECOND: i64 = 1_000_000;
const SECONDS_PER_DAY: i64 = 86_400;

/// The year, month and day of a `date` or `datetime`.
pub(crate) fn civil_date(value: &Bound<'_, PyAny>) -> PyResult<CivilDate> {
    Ok(CivilDate {
        year: native_field(value, "year")?.extract()?,
        month: native_field(value, "month")?.extract()?,
        day: native_field(value, "day")?.extract()?,
    })
}

/// A `timedelta` as whole microseconds.
pub(crate) fn timedelta_micros(delta: &Bound<'_, PyAny>) -> PyResult<i64> {
    let days: i64 = native_field(delta, "days")?.extract()?;
    let seconds: i64 = native_field(delta, "seconds")?.extract()?;
    let microseconds: i64 = native_field(delta, "microseconds")?.extract()?;
    Ok((days * SECONDS_PER_DAY + seconds) * MICROS_PER_SECOND + microseconds)
}

/// A naive `datetime` holding `local`.
pub(crate) fn naive_date_time<'py>(
    py: Python<'py>,
    local: &LocalDateTime,
    fold: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let class: Bound<'py, PyAny> = imported(py, DATETIME_MODULE, "datetime")?;
    let value: Bound<'py, PyAny> = class.call1((
        local.date.year,
        local.date.month,
        local.date.day,
        local.hour,
        local.minute,
        local.second,
        local.microsecond,
    ))?;
    if !fold {
        return Ok(value);
    }
    replaced(&value, local, fold)
}

/// A `date` holding `date`.
pub(crate) fn plain_date<'py>(py: Python<'py>, date: &CivilDate) -> PyResult<Bound<'py, PyAny>> {
    imported(py, DATETIME_MODULE, "date")?.call1((date.year, date.month, date.day))
}
