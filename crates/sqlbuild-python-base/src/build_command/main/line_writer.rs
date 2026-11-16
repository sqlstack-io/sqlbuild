//! A callable writing each message as a line on `stream`.

use crate::build_command::_helpers::support;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A callable writing each message as a line on `stream`.
#[inline]
pub fn line_writer<'py>(stream: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    support::line_writer(stream)
}
