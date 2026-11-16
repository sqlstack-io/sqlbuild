//! Maximum allowed start.

use crate::planner_resolution::_helpers::cursor::cursor_bounds::required_scalar;
use crate::planner_resolution::_helpers::cursor::cursor_safety::{horizon, safety_duration};
use crate::planner_resolution::_helpers::references::values::string;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_cursor::algebra::main::render::render;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_front::cursor::models::CursorSession;

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub fn maximum_allowed_start<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let args = KeywordArguments::read(
        py,
        "maximum_allowed_start",
        kwargs,
        (
            &[
                "discovered_value",
                "cursor_grain",
                "invocation_time",
                "max_ahead",
            ],
            &[],
        ),
    )?;
    let session = CursorSession::new(py)?;
    let discovered = required_scalar(
        &session,
        &args.value("discovered_value")?,
        &string(py, "timestamp"),
    )?;
    let duration = safety_duration(&args.value("max_ahead")?, false)?;
    session.checked(render(
        &horizon(
            &session,
            &session.python_scalar(&discovered)?,
            (
                &args.value("invocation_time")?,
                &args.value("cursor_grain")?,
            ),
            &duration,
        )?,
        &session,
    ))
}
