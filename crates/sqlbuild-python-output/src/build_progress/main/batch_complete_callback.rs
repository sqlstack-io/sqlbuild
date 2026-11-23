//! A callable that forwards each completed microbatch to `callbacks`, built without a Python lookup.

use crate::build_progress::_helpers::build_callbacks::relay;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyTupleMethods;

/// A callable that forwards each completed microbatch to `callbacks`, built without a Python lookup.
pub fn batch_complete_callback<'py>(callbacks: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    relay(callbacks, |owner, arguments| {
        owner.on_batch_complete(arguments.py(), &arguments.get_item(0)?)
    })
}
