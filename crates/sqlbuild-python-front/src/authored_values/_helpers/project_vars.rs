//! Project variable values rendered as the text SQL and config interpolation inserts.

use sqlbuild_host::entries::models::FacadeHost;
use sqlbuild_python_base::native_host::main::host_error::host_error;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyDict;

const JSON_MODULE: &str = "json";
/// Characters of a structured value's JSON preview before it is cut short.
const PREVIEW_LIMIT: usize = 120;
const PREVIEW_ELLIPSIS: &str = "...";

/// `SharedInputError(message)`.
pub(crate) fn shared_input_error(py: Python<'_>, message: String) -> PyErr {
    host_error(py, FacadeHost::SharedInputError, &message)
}

/// Compact, key-sorted JSON of `value`, cut to the preview limit.
pub(crate) fn preview_json(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let py: Python<'_> = value.py();
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item("separators", (",", ":"))?;
    keywords.set_item("sort_keys", true)?;
    let preview: String = py
        .import(JSON_MODULE)?
        .getattr("dumps")?
        .call((value,), Some(&keywords))?
        .extract()?;
    let characters: Vec<char> = preview.chars().collect();
    if characters.len() <= PREVIEW_LIMIT {
        return Ok(preview);
    }
    let kept: String = characters[..PREVIEW_LIMIT - PREVIEW_ELLIPSIS.len()]
        .iter()
        .collect();
    Ok(format!("{kept}{PREVIEW_ELLIPSIS}"))
}
