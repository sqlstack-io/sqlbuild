//! A target location that Python nodes can use outside any model.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_model_config::templates::main::holds_context_placeholder::holds_context_placeholder;

/// `location`, or `default` when it is unset, empty, or resolves only inside a model, such as a `${CTX:...}` schema.
pub fn model_free_location<'py>(
    location: Bound<'py, PyAny>,
    default: impl FnOnce() -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    match location.extract::<Option<String>>()? {
        Some(text) if !text.is_empty() && !holds_context_placeholder(&text) => Ok(location),
        _ => default(),
    }
}
