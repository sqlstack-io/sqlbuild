//! `chunk_text` for command-output records.

use pyo3::prelude::{
    Bound, PyAny, PyModule, PyModuleMethods, PyResult, Python, pyfunction, wrap_pyfunction,
};
use pyo3::types::{PyString, PyTuple};
use sqlbuild_core::command_output::main::chunk_output_text::chunk_output_text;

use crate::bindings::_helpers::output_capture::values::pass_text;
use crate::bindings::_helpers::output_capture::values::python_text;
use crate::bindings::_helpers::output_capture::values::unencodable;

/// Split text deterministically without splitting Unicode code points.
#[pyfunction]
#[pyo3(signature = (*, text, max_bytes))]
fn chunk_command_output_text<'py>(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(chunk_command_output_text, module)?)
}
