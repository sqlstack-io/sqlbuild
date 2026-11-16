//! Calendar durations parsed from text and applied to dates and timestamps.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_cursor::algebra::models::DurationSpan;

use sqlbuild_python_base::records::main::fields::field::field as native_field;

const MONTHS_PER_YEAR: i64 = 12;

pub(crate) fn duration_span(duration: &Bound<'_, PyAny>) -> PyResult<DurationSpan> {
    let field = |name: &str| -> PyResult<i64> { native_field(duration, name)?.extract() };
    Ok(DurationSpan {
        months: field("years")? * MONTHS_PER_YEAR + field("months")?,
        days: field("days")?,
        hours: field("hours")?,
        minutes: field("minutes")?,
        seconds: field("seconds")?,
    })
}
