//! The `message`, `code` and `help` text an expected error carries, such as a `CliUserError`'s.

use pyo3::PyErr;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};

use crate::boundary::main::optional_text::optional_text;

/// `error`'s non-empty `message`, `code` and `help` texts; an error without one leaves it `None`.
pub fn error_text_fields(
    py: Python<'_>,
    error: &PyErr,
) -> PyResult<(Option<String>, Option<String>, Option<String>)> {
    let value: &Bound<'_, PyAny> = error.value(py).as_any();
    Ok((
        text_field(value, "message")?,
        text_field(value, "code")?,
        text_field(value, "help")?,
    ))
}

fn text_field(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    let Some(field) = value.getattr_opt(name)? else {
        return Ok(None);
    };
    Ok(optional_text(&field)?.filter(|text| !text.trim().is_empty()))
}
