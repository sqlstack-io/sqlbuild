//! A sentinel's token, or a scalar rendered.

use crate::cursor::main::scalars::render_value::render_value;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// A sentinel's token, or a scalar rendered.
#[pyfunction(name = "cursor_sentinel_to_token")]
#[pyo3(signature = (*, sentinel))]
pub fn sentinel_to_token(sentinel: &Bound<'_, PyAny>) -> PyResult<String> {
    if sentinel.is_instance(&host_entry(sentinel.py(), ExecutorHost::BoundSentinel)?)? {
        return native_field(sentinel, "value")?.extract();
    }
    render_value(sentinel)
}
