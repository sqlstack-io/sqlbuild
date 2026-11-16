//! Shared Python lookups of the dbt commands: streams, colour, paths and status lines.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyBool;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::build_progress::main::transient_status::status_reporter::status_reporter;
use sqlbuild_python_base::presentation_text::main::stdout_supports_color::stdout_supports_color;

/// `sys.stdout` or `sys.stderr`, looked up when written.
pub(crate) fn stream<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    py.import("sys")?.getattr(name)
}

/// `supports_color()`.
pub(crate) fn color_enabled(py: Python<'_>) -> PyResult<bool> {
    stdout_supports_color(py)
}

pub(crate) fn flag(py: Python<'_>, value: bool) -> Bound<'_, PyAny> {
    PyBool::new(py, value).to_owned().into_any()
}

/// The project directory, or the working directory when none was given.
pub(crate) fn effective_dir<'py>(
    py: Python<'py>,
    project_dir: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    match project_dir.filter(|path| !path.is_none()) {
        Some(path) => Ok(path),
        None => imported(py, "pathlib", "Path")?.call_method0("cwd"),
    }
}

/// `pathlib.Path(text)`.
pub(crate) fn path<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    imported(py, "pathlib", "Path")?.call1((text,))
}

/// A transient status line on `stream`.
pub(crate) fn status<'py>(
    stream: &Bound<'py, PyAny>,
    use_color: bool,
) -> PyResult<Bound<'py, PyAny>> {
    status_reporter(stream, use_color)
}
