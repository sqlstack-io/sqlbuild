//! A failed call's error with its Python traceback, as a failed tick shows it.

use pyo3::prelude::{PyAnyMethods, PyErr, Python};

/// The error with its Python traceback, as the failed tick shows it.
pub fn sensor_traceback(py: Python<'_>, error: &PyErr) -> String {
    let formatted = py.import("traceback").and_then(|module| {
        module
            .getattr("format_exception")?
            .call1((error.value(py),))?
            .extract::<Vec<String>>()
    });
    match formatted {
        Ok(lines) => lines.concat().trim_end().to_owned(),
        Err(_) => error.to_string(),
    }
}
