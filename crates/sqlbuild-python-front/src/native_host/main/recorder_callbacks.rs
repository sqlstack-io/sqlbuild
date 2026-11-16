//! The recorder's `on_node_complete` and `before_node_write` methods, as build callbacks.

use crate::native_host::_helpers::consumption_state;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The recorder's `on_node_complete` and `before_node_write` methods, as build callbacks.
#[inline]
pub fn recorder_callbacks<'py>(
    recorder: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    consumption_state::recorder_callbacks(recorder)
}
