//! Calendar durations parsed from text and applied to dates and timestamps.

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyInt, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_cursor::algebra::main::parse_duration::parse_duration;
use sqlbuild_cursor::algebra::main::shift_by_duration::shift_by_duration;
use sqlbuild_cursor::algebra::models::{DurationDigits, DurationSpan, Moment};

use crate::bindings::_helpers::cursor::conversion::CursorSession;
use crate::bindings::_helpers::cursor::values::{
    DATETIME_MODULE, civil_date, local_date_time, replaced,
};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const MONTHS_PER_YEAR: i64 = 12;

/// `(years, months, days, hours, minutes, seconds)` of a duration like `1y6mo` or `30d`.
#[pyfunction]
#[pyo3(signature = (value))]
pub(crate) fn parse_cursor_duration<'py>(
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

fn duration_span(duration: &Bound<'_, PyAny>) -> PyResult<DurationSpan> {
    let field = |name: &str| -> PyResult<i64> { native_field(duration, name)?.extract() };
    Ok(DurationSpan {
        months: field("years")? * MONTHS_PER_YEAR + field("months")?,
        days: field("days")?,
        hours: field("hours")?,
        minutes: field("minutes")?,
        seconds: field("seconds")?,
    })
}

/// `moment` (a `date` or `datetime`) moved by `span` in `direction`, keeping its type and `tzinfo`.
pub(crate) fn shifted_moment<'py>(
    moment: &Bound<'py, PyAny>,
    span: &DurationSpan,
    direction: i64,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = moment.py();
    let session: CursorSession<'py> = CursorSession::new(py)?;
    let is_timestamp: bool = moment.is_instance(&imported(py, DATETIME_MODULE, "datetime")?)?;
    let typed: Moment = if is_timestamp {
        Moment::Timestamp {
            local: local_date_time(moment)?,
            fold: native_field(moment, "fold")?.extract::<u8>()? != 0,
        }
    } else {
        Moment::Date(civil_date(moment)?)
    };
    let shifted: Moment = session.checked(shift_by_duration(typed, span, direction))?;
    match shifted {
        Moment::Timestamp { local, fold } => replaced(moment, &local, fold),
        Moment::Date(date) => call_with(
            &native_field(moment, "replace")?,
            &[
                ("year", date.year.into_pyobject(py)?.into_any()),
                ("month", date.month.into_pyobject(py)?.into_any()),
                ("day", date.day.into_pyobject(py)?.into_any()),
            ],
        ),
    }
}

/// `moment` moved forward (`direction` 1) or back (`direction` -1) by `duration`.
#[pyfunction]
#[pyo3(signature = (*, moment, duration, direction))]
pub(crate) fn shift_by_cursor_duration<'py>(
    moment: &Bound<'py, PyAny>,
    duration: &Bound<'py, PyAny>,
    direction: i64,
) -> PyResult<Bound<'py, PyAny>> {
    shifted_moment(moment, &duration_span(duration)?, direction)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_cursor_duration, module)?)?;
    module.add_function(wrap_pyfunction!(shift_by_cursor_duration, module)?)?;
    Ok(())
}
