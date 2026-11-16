//! `moment` (a `date` or `datetime`) moved by `span` in `direction`, keeping its type and `tzinfo`.

use crate::cursor::_helpers::values::civil_date;
use crate::cursor::constants::DATETIME_MODULE;
use crate::cursor::main::values::local_date_time::local_date_time;
use crate::cursor::main::values::replaced::replaced;
use crate::cursor::models::CursorSession;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_cursor::algebra::main::shift_by_duration::shift_by_duration;
use sqlbuild_cursor::algebra::models::{DurationSpan, Moment};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `moment` (a `date` or `datetime`) moved by `span` in `direction`, keeping its type and `tzinfo`.
pub fn shifted_moment<'py>(
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
