//! `dataclasses.replace(value, **changes)`: a native record is rebuilt from its own values.

use crate::records::_helpers::native_classes::fields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `dataclasses.replace(value, **changes)`: a native record is rebuilt from its own values.
#[inline]
pub fn replaced<'py>(
    value: &Bound<'py, PyAny>,
    changes: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    fields::replaced(value, changes)
}
