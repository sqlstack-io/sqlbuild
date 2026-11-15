//! Python `date`, `datetime` and `timedelta` objects read into and built from cursor values.

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_cursor::algebra::models::{CivilDate, LocalDateTime};

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) const DATETIME_MODULE: &str = "datetime";
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

/// The wall-clock fields of a `datetime`.
pub(crate) fn local_date_time(value: &Bound<'_, PyAny>) -> PyResult<LocalDateTime> {
    Ok(LocalDateTime {
        date: civil_date(value)?,
        hour: native_field(value, "hour")?.extract()?,
        minute: native_field(value, "minute")?.extract()?,
        second: native_field(value, "second")?.extract()?,
        microsecond: native_field(value, "microsecond")?.extract()?,
    })
}

/// A `timedelta` as whole microseconds.
pub(crate) fn timedelta_micros(delta: &Bound<'_, PyAny>) -> PyResult<i64> {
    let days: i64 = native_field(delta, "days")?.extract()?;
    let seconds: i64 = native_field(delta, "seconds")?.extract()?;
    let microseconds: i64 = native_field(delta, "microseconds")?.extract()?;
    Ok((days * SECONDS_PER_DAY + seconds) * MICROS_PER_SECOND + microseconds)
}

/// `template.replace(...)` with the wall time and fold of `local`, keeping type and `tzinfo`.
pub(crate) fn replaced<'py>(
    template: &Bound<'py, PyAny>,
    local: &LocalDateTime,
    fold: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = template.py();
    call_with(
        &native_field(template, "replace")?,
        &[
            ("year", local.date.year.into_pyobject(py)?.into_any()),
            ("month", local.date.month.into_pyobject(py)?.into_any()),
            ("day", local.date.day.into_pyobject(py)?.into_any()),
            ("hour", local.hour.into_pyobject(py)?.into_any()),
            ("minute", local.minute.into_pyobject(py)?.into_any()),
            ("second", local.second.into_pyobject(py)?.into_any()),
            (
                "microsecond",
                local.microsecond.into_pyobject(py)?.into_any(),
            ),
            ("fold", u8::from(fold).into_pyobject(py)?.into_any()),
        ],
    )
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
