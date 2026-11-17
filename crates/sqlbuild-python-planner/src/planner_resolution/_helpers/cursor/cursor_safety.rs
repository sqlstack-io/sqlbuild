//! Shared safety horizon conversion and Python evidence construction.

use crate::planner_resolution::_helpers::cursor::cursor_bounds::duration;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_cursor::algebra::models::{DurationSpan, Grain, Scalar};
use sqlbuild_cursor::planner::main::safety_horizon::safety_horizon;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::cursor::models::CursorSession;

pub(crate) fn safety_error(py: Python<'_>, class: ErrorId, message: &str) -> PyErr {
    new_plain_error(py, class, message)
}

pub(crate) fn safety_duration(value: &Bound<'_, PyAny>, future: bool) -> PyResult<DurationSpan> {
    if let Some(parsed) = duration(value)? {
        return Ok(parsed);
    }
    if value.eq("0d")? {
        return Ok(DurationSpan {
            months: 0,
            days: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        });
    }
    let (class, field) = if future {
        (ErrorId::FutureCursorSafety, "future.max_distance")
    } else {
        (ErrorId::MaximumAutomaticStart, "start.max_ahead")
    };
    Err(safety_error(
        value.py(),
        class,
        &format!("invalid cursors.{field} '{}'", value.str()?),
    ))
}

pub(crate) fn timestamp_bound(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = value.py();
    Ok(value.is_instance(&record_class(py, RecordId::DateValue)?)?
        || value.is_instance(&record_class(py, RecordId::TimestampValue)?)?)
}

pub(crate) fn utc_clock<'py>(clock: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    clock.call_method1("astimezone", (imported(clock.py(), "datetime", "UTC")?,))
}

pub(crate) fn horizon<'py>(
    session: &CursorSession<'py>,
    discovered: &Bound<'py, PyAny>,
    clock_and_grain: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    span: &DurationSpan,
) -> PyResult<Scalar> {
    let py = session.py();
    let (clock, grain) = clock_and_grain;
    let mut clock = utc_clock(clock)?;
    let date_only = discovered.is_instance(&record_class(py, RecordId::DateValue)?)?;
    if date_only || native_field(&native_field(discovered, "value")?, "tzinfo")?.is_none() {
        clock = call_with(
            &native_field(&clock, "replace")?,
            &[("tzinfo", py.None().into_bound(py))],
        )?;
    }
    let clock = new_record_with(py, RecordId::TimestampValue, &[("value", clock)])?;
    let grain = if grain.is_truthy()? {
        session.grain(grain)?.unwrap_or(Grain::Second)
    } else {
        Grain::Second
    };
    session.checked(safety_horizon(
        &session.scalar(&clock)?,
        span,
        grain,
        date_only,
    ))
}

pub(crate) fn replace<'py>(
    object: &Bound<'py, PyAny>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    replaced_keywords(object, &keywords(object.py(), fields)?)
}
