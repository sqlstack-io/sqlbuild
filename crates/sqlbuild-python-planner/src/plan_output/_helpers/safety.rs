//! Structured future-cursor and maximum-start safety evidence for plan and result JSON.

use crate::plan_output::main::dict::dict;
use crate::plan_output::main::maximum_start_safety::maximum_start_safety;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyString;

use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::cursor::main::scalars::render_value::render_value;

/// `render(value=value)`.
pub(crate) fn render<'py>(value: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyString::new(value.py(), &render_value(&value)?).into_any())
}

/// `render(value=value)` when set, else `None`.
pub(crate) fn optional_render<'py>(value: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_none() {
        return Ok(value);
    }
    render(value)
}

pub(crate) fn bounds<'py>(
    evidence: &Bound<'py, PyAny>,
    (start, end): (&str, &str),
) -> PyResult<Bound<'py, PyAny>> {
    Ok(dict(
        evidence.py(),
        vec![
            ("start", render(native_field(evidence, start)?)?),
            ("end", render(native_field(evidence, end)?)?),
        ],
    )?
    .into_any())
}

/// Serialize maximum automatic-start safety evidence; `None` stays `None`.
#[pyfunction]
pub(crate) fn serialize_maximum_start_safety<'py>(
    evidence: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    maximum_start_safety(&evidence)
}
