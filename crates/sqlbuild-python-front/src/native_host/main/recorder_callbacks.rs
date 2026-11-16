//! The recorder's `on_node_complete` and `before_node_write` methods, as build callbacks.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// The recorder's `on_node_complete` and `before_node_write` methods, as build callbacks.
pub fn recorder_callbacks<'py>(
    recorder: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    Ok((
        recorder.getattr("on_node_complete")?,
        recorder.getattr("before_node_write")?,
    ))
}
