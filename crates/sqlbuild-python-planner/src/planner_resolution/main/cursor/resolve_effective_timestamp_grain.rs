//! Resolve effective timestamp grain.

use crate::planner_resolution::_helpers::references::values::string;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_front::cursor::models::CursorSession;

#[pyfunction]
#[pyo3(signature = (*, cursor_type, downstream_grain, cursor_input_grains, microbatch_strategy=None))]
pub fn resolve_effective_timestamp_grain<'py>(
    cursor_type: &Bound<'py, PyAny>,
    downstream_grain: &Bound<'py, PyAny>,
    cursor_input_grains: &Bound<'py, PyAny>,
    microbatch_strategy: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = cursor_type.py();
    if !cursor_type.eq("timestamp")? {
        return Ok(py.None().into_bound(py));
    }
    let mut effective = if downstream_grain.is_truthy()? {
        downstream_grain.clone()
    } else {
        string(py, "second")
    };
    if microbatch_strategy == Some("watermark") {
        return Ok(effective);
    }
    let session = CursorSession::new(py)?;
    for value in cursor_input_grains.try_iter()? {
        let value = value?;
        let candidate = if value.is_truthy()? {
            value
        } else {
            string(py, "second")
        };
        if session.grain(&candidate)? > session.grain(&effective)? {
            effective = candidate;
        }
    }
    Ok(effective)
}
