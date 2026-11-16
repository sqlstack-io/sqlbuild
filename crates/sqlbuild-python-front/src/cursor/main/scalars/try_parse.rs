//! Discovery values that may be dates or timestamps, or `None` when they do not parse.

use crate::cursor::_helpers::scalars;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Discovery values that may be dates or timestamps, or `None` when they do not parse.
#[inline]
pub fn try_parse<'py>(
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    scalars::try_parse(raw, cursor_type)
}
