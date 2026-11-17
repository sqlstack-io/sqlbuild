//! `chunk_text` for command-output records.

use pyo3::prelude::{Bound, PyAny, PyResult, Python, pyfunction};
use pyo3::types::{PyString, PyTuple};
use sqlbuild_core::command_output::main::chunk_output_text::chunk_output_text;

use crate::output_capture::_helpers::values::pass_text;
use crate::output_capture::_helpers::values::python_text;
use crate::output_capture::_helpers::values::unencodable;

/// Split text deterministically without splitting Unicode code points.
#[pyfunction]
#[pyo3(signature = (*, text, max_bytes))]
pub(crate) fn chunk_command_output_text<'py>(
    py: Python<'py>,
    text: &Bound<'py, PyString>,
    max_bytes: i64,
) -> PyResult<Bound<'py, PyTuple>> {
    let bytes = pass_text(text)?;
    let chunks =
        chunk_output_text(&bytes, max_bytes).map_err(|error| unencodable(py, &bytes, &error))?;
    let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
    for chunk in &chunks {
        items.push(python_text(py, chunk)?);
    }
    PyTuple::new(py, items)
}
