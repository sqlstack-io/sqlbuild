//! A `datetime` in UTC, a naive or offset-less value read as UTC.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_front::cursor::constants::DATETIME_MODULE;
use sqlbuild_python_front::state_rows::main::event_rows::in_utc::in_utc;
use sqlbuild_python_front::state_rows::main::event_rows::offset_aware::offset_aware;

/// A `datetime` in UTC, a naive or offset-less value read as UTC.
pub fn presumed_utc_datetime<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if offset_aware(value)? {
        return in_utc(value);
    }
    let changes: Bound<'py, PyDict> = PyDict::new(value.py());
    changes.set_item("tzinfo", imported(value.py(), DATETIME_MODULE, "UTC")?)?;
    value.call_method("replace", (), Some(&changes))
}
