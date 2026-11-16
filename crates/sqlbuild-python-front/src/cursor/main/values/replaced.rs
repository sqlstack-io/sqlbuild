//! `template.replace(...)` with the wall time and fold of `local`, keeping type and `tzinfo`.

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_cursor::algebra::models::LocalDateTime;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `template.replace(...)` with the wall time and fold of `local`, keeping type and `tzinfo`.
pub fn replaced<'py>(
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
