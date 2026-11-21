//! A callable that forwards a running loader's rows so far to `callbacks`, built without a Python lookup.

use crate::build_progress::_helpers::build_callbacks::relay;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyTupleMethods;

/// A callable taking a node key and its rows so far, which a worker reports as the node's progress.
pub fn rows_written_callback<'py>(callbacks: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    relay(callbacks, |owner, arguments| {
        let key: String = arguments.get_item(0)?.extract()?;
        owner.on_rows_written(arguments.py(), &key, arguments.get_item(1)?.extract()?)
    })
}
