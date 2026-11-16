//! `template.replace(...)` with the wall time and fold of `local`, keeping type and `tzinfo`.

use crate::cursor::_helpers::values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_cursor::algebra::models::LocalDateTime;

/// `template.replace(...)` with the wall time and fold of `local`, keeping type and `tzinfo`.
#[inline]
pub fn replaced<'py>(
    template: &Bound<'py, PyAny>,
    local: &LocalDateTime,
    fold: bool,
) -> PyResult<Bound<'py, PyAny>> {
    values::replaced(template, local, fold)
}
