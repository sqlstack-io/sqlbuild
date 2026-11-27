//! A Python exception from a load session, as the failure kind SQLLoad acts on.

use pyo3::prelude::{PyAnyMethods, PyErr, PyTypeMethods, Python};
use sqlload::adapters::session::errors::SessionError;

use crate::sessions::constants::NOT_FOUND_ERROR;

/// `error` as SQLLoad's session error: `not_found` for a driver's `NotFound`, else a failure a rerun meets again.
pub(crate) fn session_error(py: Python<'_>, error: &PyErr) -> SessionError {
    let message: String = error
        .value(py)
        .str()
        .map_or_else(|_| error.to_string(), |text| text.to_string());
    let class: String = error
        .get_type(py)
        .name()
        .map_or_else(|_| String::new(), |name| name.to_string());
    match class == NOT_FOUND_ERROR {
        true => SessionError::not_found(message),
        false => SessionError::new(message),
    }
}
